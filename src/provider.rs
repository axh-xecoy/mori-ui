use crate::{
    model::*,
    storage::{Store, api_key, data_url},
    tools::ToolRegistry,
};
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub enum Event {
    Message(Message),
    Approval(ToolCall, mpsc::Sender<bool>),
    Status(String),
    Error(String),
    Done,
}
#[derive(Clone)]
pub struct Request {
    pub provider: Provider,
    pub conversation: Conversation,
    pub prompt: String,
    pub settings: Settings,
}

fn client() -> Result<Client> {
    Ok(Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()?)
}
pub fn normalize_base_url(protocol: &Protocol, url: &str) -> String {
    let base = url.trim().trim_end_matches('/');
    if *protocol == Protocol::Ollama {
        // Native Ollama uses /api endpoints; /v1 belongs to its OpenAI adapter.
        base.strip_suffix("/v1")
            .or_else(|| base.strip_suffix("/api"))
            .unwrap_or(base)
            .to_string()
    } else {
        base.to_string()
    }
}
fn authorize(request: RequestBuilder, provider: &Provider) -> Result<RequestBuilder> {
    let key = api_key(provider)?;
    Ok(if key.is_empty() {
        request
    } else if provider.protocol == Protocol::Anthropic {
        request
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
    } else {
        request.bearer_auth(key)
    })
}
fn checked(response: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().unwrap_or_default();
    bail!(
        "服务商返回 {status}：{}",
        body.chars().take(1500).collect::<String>()
    )
}
pub fn models(provider: &Provider) -> Result<Vec<String>> {
    let endpoint = if provider.protocol == Protocol::Ollama {
        "api/tags"
    } else {
        "models"
    };
    let response: Value = checked(
        authorize(
            client()?.get(format!(
                "{}/{endpoint}",
                normalize_base_url(&provider.protocol, &provider.base_url)
            )),
            provider,
        )?
        .send()?,
    )?
    .json()?;
    let items = if provider.protocol == Protocol::Ollama {
        &response["models"]
    } else {
        &response["data"]
    };
    let mut models: Vec<String> = items
        .as_array()
        .context("模型列表格式不正确")?
        .iter()
        .filter_map(|v| {
            v[if provider.protocol == Protocol::Ollama {
                "name"
            } else {
                "id"
            }]
            .as_str()
            .map(str::to_string)
        })
        .collect();
    models.sort();
    models.dedup();
    Ok(models)
}

pub fn messages(protocol: &Protocol, history: &[Message], prompt: &str) -> Result<Vec<Value>> {
    let mut result = vec![];
    if *protocol != Protocol::Anthropic && !prompt.is_empty() {
        result.push(json!({"role":"system","content":prompt}));
    }
    for message in history {
        if message.error.is_some() && message.content.is_empty() && message.tools.is_empty() {
            continue;
        }
        let mut text = message.content.clone();
        for a in &message.attachments {
            if let Some(content) = &a.text
                && (a.mime != "application/pdf" || *protocol == Protocol::Ollama)
            {
                text.push_str(&format!(
                    "\n\n<document name={:?}>\n{}\n</document>",
                    a.name, content
                ));
            }
        }
        match protocol {
            Protocol::Ollama => {
                let mut item = json!({"role":message.role,"content":text});
                let mut images = vec![];
                for a in &message.attachments {
                    if a.mime.starts_with("image/") {
                        images.push(STANDARD.encode(std::fs::read(&a.path)?));
                    }
                    if a.mime == "application/pdf"
                        && a.text.as_ref().is_none_or(|s| s.trim().is_empty())
                    {
                        bail!("该 PDF 没有可提取的文字；扫描件请使用支持 PDF 视觉输入的云端模型");
                    }
                }
                if !images.is_empty() {
                    item["images"] = json!(images);
                }
                if !message.tools.is_empty() {
                    item["tool_calls"] = json!(message.tools.iter().map(|t| Ok(json!({"function":{"name":t.name,"arguments":serde_json::from_str::<Value>(&t.arguments)?}}))).collect::<Result<Vec<_>>>()?);
                }
                result.push(item);
                for t in &message.tools {
                    if let Some(output) = &t.result {
                        result.push(json!({"role":"tool","tool_name":t.name,"content":output}));
                    }
                }
            }
            Protocol::OpenAi => {
                let mut content = vec![json!({"type":"text","text":text})];
                for a in &message.attachments {
                    if a.mime.starts_with("image/") {
                        content.push(json!({"type":"image_url","image_url":{"url":data_url(a)?}}));
                    }
                    if a.mime == "application/pdf" {
                        content.push(json!({"type":"file","file":{"filename":a.name,"file_data":data_url(a)?}}));
                    }
                }
                let mut item = json!({"role":message.role,"content":if content.len()==1 { content[0]["text"].clone() } else { json!(content) }});
                if !message.reasoning.is_empty() {
                    item["reasoning_content"] = json!(message.reasoning);
                }
                if !message.tools.is_empty() {
                    item["tool_calls"] = json!(message.tools.iter().map(|t| json!({"id":t.id,"type":"function","function":{"name":t.name,"arguments":t.arguments}})).collect::<Vec<_>>());
                }
                result.push(item);
                for t in &message.tools {
                    if let Some(output) = &t.result {
                        result.push(json!({"role":"tool","tool_call_id":t.id,"content":output}));
                    }
                }
            }
            Protocol::Anthropic => {
                let mut content = message.thinking_blocks.clone();
                if !text.is_empty() {
                    content.push(json!({"type":"text","text":text}));
                }
                for a in &message.attachments {
                    if a.text.is_some() && a.mime != "application/pdf" {
                        continue;
                    }
                    content.push(json!({"type":if a.mime == "application/pdf" {"document"} else {"image"},"source":{"type":"base64","media_type":a.mime,"data":STANDARD.encode(std::fs::read(&a.path)?)}}));
                }
                for t in &message.tools {
                    content.push(json!({"type":"tool_use","id":t.id,"name":t.name,"input":serde_json::from_str::<Value>(&t.arguments)?}));
                }
                if !content.is_empty() {
                    result.push(json!({"role":message.role,"content":content}));
                }
                let tool_results: Vec<Value> = message
                    .tools
                    .iter()
                    .filter_map(|t| {
                        t.result
                            .as_ref()
                            .map(|r| json!({"type":"tool_result","tool_use_id":t.id,"content":r}))
                    })
                    .collect();
                if !tool_results.is_empty() {
                    result.push(json!({"role":"user","content":tool_results}));
                }
            }
        }
    }
    Ok(result)
}

fn body(request: &Request, history: &[Message], tools: &[Value]) -> Result<Value> {
    let mut body = json!({"model":request.conversation.model,"messages":messages(&request.provider.protocol, history, &request.prompt)?,"stream":true});
    match request.provider.protocol {
        Protocol::Ollama => {
            body["options"] = json!({"temperature":request.settings.temperature,"num_predict":if request.settings.max_tokens == 0 { -1i64 } else { i64::from(request.settings.max_tokens) }});
        }
        Protocol::Anthropic => {
            body["system"] = json!(request.prompt);
            if request.settings.max_tokens > 0 {
                body["max_tokens"] = json!(request.settings.max_tokens);
            }
            body["temperature"] = json!(request.settings.temperature);
            if !tools.is_empty() {
                body["tools"] = json!(tools.iter().map(|t| json!({"name":t["function"]["name"],"description":t["function"]["description"],"input_schema":t["function"]["parameters"]})).collect::<Vec<_>>());
            }
        }
        Protocol::OpenAi => {
            body["stream_options"] = json!({"include_usage":true});
            // OpenAI reasoning models do not accept temperature or legacy max_tokens.
            let model = request.conversation.model.rsplit('/').next().unwrap_or("");
            let reasoning = model.starts_with("o1")
                || model.starts_with("o3")
                || model.starts_with("o4")
                || model.starts_with("gpt-5")
                || model.starts_with("gpt-6");
            if reasoning {
                if request.settings.max_tokens > 0 {
                    body["max_completion_tokens"] = json!(request.settings.max_tokens);
                }
            } else {
                body["temperature"] = json!(request.settings.temperature);
                if request.settings.max_tokens > 0 {
                    body["max_tokens"] = json!(request.settings.max_tokens);
                }
            }
        }
    }
    if let Some(enabled) = request.settings.thinking {
        let url = request.provider.base_url.to_lowercase();
        let model = request.conversation.model.to_lowercase();
        match request.provider.protocol {
            Protocol::Ollama => body["think"] = json!(enabled),
            Protocol::Anthropic => {
                if enabled {
                    if model.contains("4-6") || model.contains("4-7") {
                        body["thinking"] = json!({"type":"adaptive"});
                    } else {
                        if request.settings.max_tokens > 0 && request.settings.max_tokens < 2048 {
                            bail!("开启思考需要至少 2048 个输出 token");
                        }
                        body["thinking"] = json!({"type":"enabled","budget_tokens":if request.settings.max_tokens == 0 {4096} else {(request.settings.max_tokens / 2).clamp(1024, 4096)}});
                    }
                    body.as_object_mut().unwrap().remove("temperature");
                } else {
                    body["thinking"] = json!({"type":"disabled"});
                }
            }
            Protocol::OpenAi => {
                if url.contains("openrouter.ai") {
                    body["reasoning"] = json!({"enabled":enabled});
                } else if url.contains("deepseek.com") {
                    body["thinking"] = json!({"type":if enabled {"enabled"} else {"disabled"}});
                } else if model.contains("qwen")
                    || url.contains("dashscope")
                    || url.contains("siliconflow")
                {
                    body["enable_thinking"] = json!(enabled);
                } else if model.starts_with("gpt-5.1") || model.starts_with("gpt-5.2") {
                    body["reasoning_effort"] = json!(if enabled { "medium" } else { "none" });
                } else if model.starts_with("o1")
                    || model.starts_with("o3")
                    || model.starts_with("o4")
                    || model.starts_with("gpt-5")
                    || model.starts_with("gpt-6")
                {
                    if !enabled {
                        bail!("此模型无法保证关闭思考，请选择支持非思考模式的模型或使用自动模式");
                    }
                    body["reasoning_effort"] = json!("medium");
                } else {
                    bail!("当前模型或服务商尚未适配思考开关，请使用自动模式或选择支持的模型");
                }
            }
        }
    }
    if !tools.is_empty() && request.provider.protocol != Protocol::Anthropic {
        body["tools"] = json!(tools);
    }
    apply_parameters(
        &mut body,
        &request.settings.request_parameters,
        &request.provider.protocol,
    )?;
    if let Some(config) = request
        .provider
        .model_configs
        .get(&request.conversation.model)
    {
        apply_parameters(&mut body, &config.parameters, &request.provider.protocol)?;
    }
    if request.provider.protocol == Protocol::Anthropic {
        let max = body["max_tokens"]
            .as_u64()
            .filter(|v| *v > 0)
            .context("Claude 接口需要设置正整数 max_tokens，请在模型高级参数中填写输出上限")?;
        if let Some(budget) = body["thinking"]["budget_tokens"].as_u64()
            && budget >= max
        {
            bail!("思考预算 budget_tokens 必须小于 max_tokens，请调整模型高级参数");
        }
    }
    Ok(body)
}

pub fn validate_parameters(parameters: &serde_json::Map<String, Value>) -> Result<()> {
    for key in ["model", "messages", "stream", "tools", "system"] {
        if parameters.contains_key(key) {
            bail!("{key} 由会话管理，不能作为模型参数覆盖");
        }
    }
    for key in ["temperature", "top_p"] {
        if let Some(value) = parameters.get(key)
            && !value.is_null()
        {
            let number = value.as_f64().context(format!("{key} 必须是数字或 null"))?;
            if !number.is_finite() || number < 0. || (key == "top_p" && number > 1.) {
                bail!("{key} 数值无效");
            }
        }
    }
    for key in ["max_tokens", "max_completion_tokens"] {
        if let Some(value) = parameters.get(key)
            && !value.is_null()
            && !value.as_u64().is_some_and(|v| v > 0)
        {
            bail!("{key} 必须是正整数或 null");
        }
    }
    if let Some(value) = parameters.get("options")
        && !value.is_null()
        && !value.is_object()
    {
        bail!("options 必须是 JSON 对象或 null");
    }
    Ok(())
}
fn merge_parameter(target: &mut Value, patch: &Value) {
    if let (Some(target), Some(patch)) = (target.as_object_mut(), patch.as_object()) {
        for (key, value) in patch {
            if value.is_null() {
                target.remove(key);
            } else {
                merge_parameter(target.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
    } else {
        *target = patch.clone();
    }
}
fn apply_parameters(
    body: &mut Value,
    parameters: &serde_json::Map<String, Value>,
    protocol: &Protocol,
) -> Result<()> {
    validate_parameters(parameters)?;
    for (key, value) in parameters {
        let mapped = if *protocol == Protocol::Ollama {
            match key.as_str() {
                "max_tokens" | "max_completion_tokens" => Some("num_predict"),
                "temperature" | "top_p" | "top_k" | "min_p" | "seed" | "repeat_penalty"
                | "num_ctx" | "num_predict" | "stop" | "presence_penalty" | "frequency_penalty"
                | "repeat_last_n" => Some(key.as_str()),
                _ => None,
            }
        } else {
            None
        };
        if let Some(mapped) = mapped {
            if !body["options"].is_object() {
                body["options"] = json!({});
            }
            if value.is_null() {
                body["options"].as_object_mut().unwrap().remove(mapped);
            } else {
                body["options"][mapped] = value.clone();
            }
        } else {
            let model = body["model"]
                .as_str()
                .unwrap_or("")
                .rsplit('/')
                .next()
                .unwrap_or("");
            let reasoning = ["o1", "o3", "o4", "gpt-5", "gpt-6"]
                .iter()
                .any(|prefix| model.starts_with(prefix));
            let mapped = if key == "max_tokens" && *protocol == Protocol::OpenAi && reasoning {
                "max_completion_tokens"
            } else {
                key
            };
            if value.is_null() {
                body.as_object_mut().unwrap().remove(mapped);
            } else {
                merge_parameter(
                    body.as_object_mut()
                        .unwrap()
                        .entry(mapped)
                        .or_insert(Value::Null),
                    value,
                );
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct Accumulator {
    calls: std::collections::BTreeMap<usize, ToolCall>,
    output: String,
    reasoning: String,
    thinking_blocks: std::collections::BTreeMap<usize, Value>,
    tokens: u64,
    finished: bool,
}
impl Accumulator {
    fn ingest(&mut self, protocol: &Protocol, value: Value) -> Result<()> {
        if let Some(error) = value.get("error") {
            bail!("流式请求失败：{error}");
        }
        match protocol {
            Protocol::Ollama => {
                self.output
                    .push_str(value["message"]["content"].as_str().unwrap_or(""));
                self.reasoning
                    .push_str(value["message"]["thinking"].as_str().unwrap_or(""));
                if let Some(calls) = value["message"]["tool_calls"].as_array() {
                    for c in calls {
                        let i = c["function"]["index"]
                            .as_u64()
                            .map(|v| v as usize)
                            .unwrap_or(self.calls.len());
                        self.calls.insert(
                            i,
                            ToolCall {
                                id: id(),
                                name: c["function"]["name"].as_str().unwrap_or("").into(),
                                arguments: c["function"]["arguments"].to_string(),
                                result: None,
                            },
                        );
                    }
                }
                self.tokens = value["eval_count"].as_u64().unwrap_or(self.tokens);
                self.finished |= value["done"] == true;
            }
            Protocol::OpenAi => {
                let delta = &value["choices"][0]["delta"];
                self.output
                    .push_str(delta["content"].as_str().unwrap_or(""));
                self.reasoning.push_str(
                    delta["reasoning_content"]
                        .as_str()
                        .or(delta["reasoning"].as_str())
                        .unwrap_or(""),
                );
                if let Some(calls) = delta["tool_calls"].as_array() {
                    for c in calls {
                        let call = self
                            .calls
                            .entry(c["index"].as_u64().unwrap_or(0) as usize)
                            .or_default();
                        if let Some(id) = c["id"].as_str() {
                            call.id.push_str(id);
                        }
                        if let Some(name) = c["function"]["name"].as_str() {
                            call.name.push_str(name);
                        }
                        if let Some(args) = c["function"]["arguments"].as_str() {
                            call.arguments.push_str(args);
                        }
                    }
                }
                self.tokens = value["usage"]["completion_tokens"]
                    .as_u64()
                    .unwrap_or(self.tokens);
                self.finished |= value["choices"][0]["finish_reason"].is_string();
            }
            Protocol::Anthropic => {
                let index = value["index"].as_u64().unwrap_or(0) as usize;
                match value["type"].as_str().unwrap_or("") {
                    "content_block_start" => {
                        let block = &value["content_block"];
                        if matches!(
                            block["type"].as_str(),
                            Some("thinking" | "redacted_thinking")
                        ) {
                            self.thinking_blocks.insert(index, block.clone());
                        }
                        if block["type"] == "tool_use" {
                            self.calls.insert(
                                index,
                                ToolCall {
                                    id: block["id"].as_str().unwrap_or("").into(),
                                    name: block["name"].as_str().unwrap_or("").into(),
                                    arguments: String::new(),
                                    result: None,
                                },
                            );
                        }
                        if block["type"] == "text" {
                            self.output.push_str(block["text"].as_str().unwrap_or(""));
                        }
                    }
                    "content_block_delta" => {
                        let delta = &value["delta"];
                        if let Some(block) = self.thinking_blocks.get_mut(&index) {
                            for key in ["thinking", "signature"] {
                                if let Some(part) = delta[key].as_str() {
                                    let mut text = block[key].as_str().unwrap_or("").to_string();
                                    text.push_str(part);
                                    block[key] = json!(text);
                                }
                            }
                        }
                        self.output.push_str(delta["text"].as_str().unwrap_or(""));
                        self.reasoning
                            .push_str(delta["thinking"].as_str().unwrap_or(""));
                        if let Some(args) = delta["partial_json"].as_str()
                            && let Some(call) = self.calls.get_mut(&index)
                        {
                            call.arguments.push_str(args);
                        }
                    }
                    "message_delta" => {
                        self.tokens = value["usage"]["output_tokens"]
                            .as_u64()
                            .unwrap_or(self.tokens);
                    }
                    "message_stop" => self.finished = true,
                    _ => {}
                }
            }
        }
        Ok(())
    }
    fn apply(&self, message: &mut Message) {
        message.content.clone_from(&self.output);
        message.reasoning.clone_from(&self.reasoning);
        message.thinking_blocks = self.thinking_blocks.values().cloned().collect();
        message.tools = self
            .calls
            .values()
            .cloned()
            .map(|mut t| {
                if t.id.is_empty() {
                    t.id = id();
                }
                if t.arguments.is_empty() {
                    t.arguments = "{}".into();
                }
                t
            })
            .collect();
        message.tokens = self.tokens;
    }
}

fn stream(
    request: &Request,
    history: &[Message],
    tools: &[Value],
    message: &mut Message,
    tx: &mpsc::Sender<Event>,
    cancel: &AtomicBool,
) -> Result<()> {
    let endpoint = match request.provider.protocol {
        Protocol::Ollama => "api/chat",
        Protocol::Anthropic => "messages",
        Protocol::OpenAi => "chat/completions",
    };
    let mut req = client()?
        .post(format!(
            "{}/{endpoint}",
            normalize_base_url(&request.provider.protocol, &request.provider.base_url)
        ))
        .json(&body(request, history, tools)?);
    if request.provider.protocol == Protocol::Anthropic {
        req = req.header("anthropic-version", "2023-06-01");
    }
    let response = checked(authorize(req, &request.provider)?.send()?)?;
    let start = Instant::now();
    let mut accumulator = Accumulator::default();
    let mut data = String::new();
    let mut published = Instant::now() - Duration::from_millis(80);
    for line in BufReader::new(response).lines() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let line = line?;
        let payload = if request.provider.protocol == Protocol::Ollama {
            if line.trim().is_empty() {
                continue;
            }
            Some(line)
        } else if line.is_empty() {
            if data.is_empty() {
                continue;
            }
            Some(std::mem::take(&mut data))
        } else {
            if let Some(part) = line.strip_prefix("data:") {
                data.push_str(part.trim_start());
                data.push('\n');
            }
            None
        };
        if let Some(payload) = payload {
            if payload.trim() == "[DONE]" {
                accumulator.finished = true;
                break;
            }
            accumulator.ingest(
                &request.provider.protocol,
                serde_json::from_str(payload.trim())?,
            )?;
            accumulator.apply(message);
            message.elapsed_ms = start.elapsed().as_millis() as u64;
            if published.elapsed() >= Duration::from_millis(80) {
                if tx.send(Event::Message(message.clone())).is_err() {
                    return Ok(());
                }
                published = Instant::now();
            }
        }
    }
    if !data.trim().is_empty() && data.trim() != "[DONE]" {
        accumulator.ingest(
            &request.provider.protocol,
            serde_json::from_str(data.trim())?,
        )?;
    }
    accumulator.apply(message);
    message.elapsed_ms = start.elapsed().as_millis() as u64;
    let _ = tx.send(Event::Message(message.clone()));
    if !accumulator.finished {
        bail!("连接在回复结束前断开；已保留收到的内容，可以重试");
    }
    Ok(())
}

pub fn run(request: Request, tx: mpsc::Sender<Event>, cancel: Arc<AtomicBool>) {
    let outcome = (|| -> Result<()> {
        let _ = tx.send(Event::Status("正在连接模型…".into()));
        let mut registry = ToolRegistry::load(&request.settings)?;
        let mut history = request.conversation.messages.clone();
        for message in &mut history {
            message.tools.retain(|t| {
                t.result.is_some() && serde_json::from_str::<Value>(&t.arguments).is_ok()
            });
        }
        for _ in 0..12 {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            let mut message = Message::new("assistant", "");
            message.model_id = request.conversation.model.clone();
            message.provider_id = request.provider.id.clone();
            stream(
                &request,
                &history,
                &registry.schemas,
                &mut message,
                &tx,
                &cancel,
            )?;
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            if message.tools.is_empty() {
                return Ok(());
            }
            for i in 0..message.tools.len() {
                let approved =
                    if request.settings.bash_trusted && registry.is_bash(&message.tools[i].name) {
                        true
                    } else {
                        let (approval_tx, approval_rx) = mpsc::channel();
                        tx.send(Event::Approval(message.tools[i].clone(), approval_tx))?;
                        loop {
                            if cancel.load(Ordering::Relaxed) {
                                return Ok(());
                            }
                            match approval_rx.recv_timeout(Duration::from_millis(100)) {
                                Ok(v) => break v,
                                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                                Err(_) => {}
                            }
                        }
                    };
                let output = if approved {
                    registry
                        .execute(&message.tools[i], &request.settings, &cancel)
                        .unwrap_or_else(|e| format!("工具执行失败：{e:#}"))
                } else {
                    "用户拒绝执行此工具。请继续回答，勿重复请求。".into()
                };
                message.tools[i].result = Some(output);
                let _ = tx.send(Event::Message(message.clone()));
            }
            history.push(message);
        }
        bail!("已达到 12 轮工具调用上限，请发送新消息继续")
    })();
    if let Err(e) = outcome {
        let _ = tx.send(Event::Error(format!("{e:#}")));
    }
    let _ = tx.send(Event::Done);
}

pub fn generate_image(
    provider: &Provider,
    prompt: &str,
    model: &str,
    store: &Store,
) -> Result<Attachment> {
    if provider.protocol != Protocol::OpenAi {
        bail!("图片生成需要支持 images/generations 的 OpenAI 兼容服务商");
    }
    let mut body = json!({"model":model,"prompt":prompt,"n":1,"size":"1024x1024"});
    if !model.starts_with("gpt-image") {
        body["response_format"] = json!("b64_json");
    }
    let value: Value = checked(
        authorize(
            client()?
                .post(format!(
                    "{}/images/generations",
                    provider.base_url.trim_end_matches('/')
                ))
                .json(&body),
            provider,
        )?
        .send()?,
    )?
    .json()?;
    let data = if let Some(data) = value["data"][0]["b64_json"].as_str() {
        STANDARD.decode(data)?
    } else if let Some(url) = value["data"][0]["url"].as_str() {
        checked(client()?.get(url).send()?)?.bytes()?.to_vec()
    } else {
        bail!("图片响应中没有图像数据：{value}");
    };
    let path = store.save_media(&data, "png")?;
    Ok(Attachment {
        name: "生成的图片.png".into(),
        path: path.to_string_lossy().into(),
        mime: "image/png".into(),
        text: None,
    })
}
pub fn transcribe(provider: &Provider, path: &std::path::Path, model: &str) -> Result<String> {
    if provider.protocol != Protocol::OpenAi {
        bail!("音频转写需要支持 audio/transcriptions 的 OpenAI 兼容服务商");
    }
    let form = reqwest::blocking::multipart::Form::new()
        .text("model", model.to_string())
        .file("file", path)?;
    let response: Value = checked(
        authorize(
            client()?
                .post(format!(
                    "{}/audio/transcriptions",
                    provider.base_url.trim_end_matches('/')
                ))
                .multipart(form),
            provider,
        )?
        .send()?,
    )?
    .json()?;
    Ok(response["text"]
        .as_str()
        .context("转写响应缺少 text")?
        .into())
}
pub fn speak(
    provider: &Provider,
    text: &str,
    model: &str,
    store: &Store,
) -> Result<std::path::PathBuf> {
    if provider.protocol != Protocol::OpenAi {
        bail!("朗读需要支持 audio/speech 的 OpenAI 兼容服务商");
    }
    let response = checked(
        authorize(
            client()?
                .post(format!(
                    "{}/audio/speech",
                    provider.base_url.trim_end_matches('/')
                ))
                .json(&json!({"model":model,"input":text,"voice":"alloy","response_format":"mp3"})),
            provider,
        )?
        .send()?,
    )?;
    store.save_media(&response.bytes()?, "mp3")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Response, server};
    #[test]
    fn model_parameters_override_globals_and_follow_native_protocols() {
        for protocol in [Protocol::OpenAi, Protocol::Ollama, Protocol::Anthropic] {
            let mut req = request(protocol.clone(), "http://localhost".into());
            req.settings.max_tokens = 0;
            req.settings.request_parameters =
                serde_json::from_value(json!({"top_p":0.5,"options":{"num_ctx":32000,"seed":2}}))
                    .unwrap();
            let config=ModelConfig {name:"千问3 8B".into(),parameters:serde_json::from_value(json!({"temperature":0.4,"top_p":0.9,"max_tokens":32768,"options":{"seed":42},"stop":["END"],"custom_vendor_flag":true})).unwrap()};
            req.provider.model_configs.insert("fixture".into(), config);
            let value = body(&req, &[], &[]).unwrap();
            assert_eq!(value["model"], "fixture");
            assert_eq!(
                if protocol == Protocol::Ollama {
                    &value["options"]["stop"]
                } else {
                    &value["stop"]
                },
                &json!(["END"])
            );
            assert_eq!(value["custom_vendor_flag"], true);
            assert_eq!(value["options"]["seed"], 42);
            assert_eq!(value["options"]["num_ctx"], 32000);
            let sample = if protocol == Protocol::Ollama {
                &value["options"]
            } else {
                &value
            };
            assert_eq!(sample["temperature"], 0.4);
            assert_eq!(sample["top_p"], 0.9);
            assert_eq!(
                sample[if protocol == Protocol::Ollama {
                    "num_predict"
                } else {
                    "max_tokens"
                }],
                32768
            );
        }
    }
    #[test]
    fn default_limits_are_optional_and_reasoning_parameters_use_completion_tokens() {
        let mut req = request(Protocol::OpenAi, "http://localhost".into());
        req.settings = Settings::default();
        let value = body(&req, &[], &[]).unwrap();
        assert!(value.get("max_tokens").is_none());
        req.provider.protocol = Protocol::Ollama;
        assert_eq!(body(&req, &[], &[]).unwrap()["options"]["num_predict"], -1);
        req.provider.protocol = Protocol::Anthropic;
        assert!(
            body(&req, &[], &[])
                .unwrap_err()
                .to_string()
                .contains("max_tokens")
        );
        req.provider.protocol = Protocol::OpenAi;
        req.conversation.model = "o3".into();
        req.settings.request_parameters = serde_json::from_value(
            json!({"max_tokens":50000,"temperature":null,"reasoning_effort":"high"}),
        )
        .unwrap();
        let value = body(&req, &[], &[]).unwrap();
        assert_eq!(value["max_completion_tokens"], 50000);
        assert!(value.get("max_tokens").is_none());
        assert!(value.get("temperature").is_none());
        req.settings.request_parameters = serde_json::from_value(json!({"stream":false})).unwrap();
        assert!(body(&req, &[], &[]).is_err());
    }
    fn request(protocol: Protocol, base_url: String) -> Request {
        let provider = Provider {
            id: id(),
            name: "Fixture".into(),
            protocol,
            base_url,
            models: vec!["fixture".into()],
            model_configs: Default::default(),
            enabled: true,
        };
        let mut conversation =
            Conversation::new(provider.id.clone(), "fixture".into(), "general".into());
        conversation.messages.push(Message::new("user", "你好"));
        Request {
            provider,
            conversation,
            prompt: "Be helpful".into(),
            settings: Settings {
                max_tokens: 16384,
                ..Settings::default()
            },
        }
    }
    #[test]
    fn full_http_streams_for_all_three_protocols() {
        for protocol in [Protocol::OpenAi, Protocol::Ollama, Protocol::Anthropic] {
            let response = match protocol {
                Protocol::OpenAi => Response::sse(&[
                    json!({"choices":[{"delta":{"content":"你好，世界"}}]}),
                    json!({"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"completion_tokens":4}}),
                ]),
                Protocol::Ollama => Response {
                    content_type: "application/x-ndjson",
                    body: format!(
                        "{}\n{}\n",
                        json!({"message":{"content":"你好，"},"done":false}),
                        json!({"message":{"content":"世界"},"done":true,"eval_count":4})
                    ),
                    extra_headers: String::new(),
                },
                Protocol::Anthropic => Response::sse(&[
                    json!({"type":"content_block_delta","index":0,"delta":{"text":"你好，世界"}}),
                    json!({"type":"message_delta","usage":{"output_tokens":4}}),
                    json!({"type":"message_stop"}),
                ]),
            };
            let (url, requests, handle) = server(vec![response]);
            let req = request(protocol.clone(), url);
            let (tx, rx) = mpsc::channel();
            run(req, tx, Arc::new(AtomicBool::new(false)));
            let events: Vec<_> = rx.try_iter().collect();
            assert!(!events.iter().any(|e| matches!(e, Event::Error(_))));
            let last = events
                .iter()
                .filter_map(|e| {
                    if let Event::Message(m) = e {
                        Some(m)
                    } else {
                        None
                    }
                })
                .next_back()
                .unwrap();
            assert_eq!(last.content, "你好，世界");
            assert_eq!(last.model_id, "fixture");
            assert!(!last.provider_id.is_empty());
            assert_eq!(last.tokens, 4);
            let (header, body) = requests.recv().unwrap();
            assert!(header.contains(match protocol {
                Protocol::OpenAi => "/chat/completions",
                Protocol::Ollama => "/api/chat",
                Protocol::Anthropic => "/messages",
            }));
            assert_eq!(body["stream"], true);
            handle.join().unwrap();
        }
    }
    #[test]
    fn ollama_native_base_urls_keep_proxy_prefixes_and_openai_is_unchanged() {
        for input in [
            "http://localhost:11434",
            "http://localhost:11434/v1/",
            " http://localhost:11434/api/ ",
        ] {
            assert_eq!(
                normalize_base_url(&Protocol::Ollama, input),
                "http://localhost:11434"
            );
        }
        assert_eq!(
            normalize_base_url(&Protocol::Ollama, "https://example.com/proxy/ollama/api"),
            "https://example.com/proxy/ollama"
        );
        assert_eq!(
            normalize_base_url(&Protocol::OpenAi, "http://localhost:11434/v1/"),
            "http://localhost:11434/v1"
        );
    }
    #[test]
    fn ollama_legacy_v1_and_api_addresses_use_native_models_and_chat_paths() {
        use crate::test_support::{Response, server};
        for suffix in ["/v1/", "/api/"] {
            let (url, requests, handle) = server(vec![
                Response::json(json!({"models":[{"name":"local-model"}]})),
                Response {
                    content_type: "application/x-ndjson",
                    body: format!(
                        "{}\n",
                        json!({"message":{"content":"正常回复"},"done":true})
                    ),
                    extra_headers: String::new(),
                },
            ]);
            let req = request(Protocol::Ollama, format!("{url}{suffix}"));
            assert_eq!(models(&req.provider).unwrap(), vec!["local-model"]);
            let (tx, rx) = mpsc::channel();
            run(req, tx, Arc::new(AtomicBool::new(false)));
            assert!(!rx.try_iter().any(|e| matches!(e, Event::Error(_))));
            assert!(requests.recv().unwrap().0.starts_with("GET /api/tags "));
            assert!(requests.recv().unwrap().0.starts_with("POST /api/chat "));
            handle.join().unwrap();
        }
    }
    #[test]
    fn denied_tool_is_returned_to_model_before_followup() {
        for trust_non_bash in [false, true] {
            let (url, requests, handle) = server(vec![
                Response::sse(&[
                    json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"t1","function":{"name":"bash","arguments":"{\"command\":\"echo hello\"}"}}]},"finish_reason":"tool_calls"}]}),
                ]),
                Response::sse(&[
                    json!({"choices":[{"delta":{"content":"无需执行工具也可以继续。"},"finish_reason":"stop"}]}),
                ]),
            ]);
            let mut req = request(Protocol::OpenAi, url);
            req.settings.tools_enabled = true;
            req.settings.bash_enabled = !trust_non_bash;
            req.settings.bash_trusted = trust_non_bash;
            if trust_non_bash {
                req.settings.functions.push(FunctionConfig {
                    name: "bash".into(),
                    description: "A custom tool with a misleading name".into(),
                    parameters: json!({"type":"object"}),
                    command: "must-never-execute".into(),
                    args: vec![],
                    enabled: true,
                });
            }
            let (tx, rx) = mpsc::channel();
            let worker = std::thread::spawn(move || run(req, tx, Arc::new(AtomicBool::new(false))));
            let mut saw_approval = false;
            let mut final_text = String::new();
            loop {
                match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
                    Event::Approval(call, tx) => {
                        assert_eq!(call.name, "bash");
                        saw_approval = true;
                        tx.send(false).unwrap();
                    }
                    Event::Error(e) => panic!("{e}"),
                    Event::Message(m) => final_text = m.content,
                    Event::Done => break,
                    _ => {}
                }
            }
            assert!(saw_approval);
            assert_eq!(final_text, "无需执行工具也可以继续。");
            let _ = requests.recv().unwrap();
            let (_, body) = requests.recv().unwrap();
            let msgs = body["messages"].as_array().unwrap();
            assert_eq!(msgs.last().unwrap()["role"], "tool");
            assert!(
                msgs.last().unwrap()["content"]
                    .as_str()
                    .unwrap()
                    .contains("拒绝")
            );
            worker.join().unwrap();
            handle.join().unwrap();
        }
    }
    #[test]
    fn thinking_controls_follow_provider_protocol_and_auto_keeps_old_requests() {
        let mut req = request(Protocol::Ollama, "http://localhost:11434".into());
        assert!(body(&req, &[], &[]).unwrap().get("think").is_none());
        for enabled in [true, false] {
            req.settings.thinking = Some(enabled);
            assert_eq!(body(&req, &[], &[]).unwrap()["think"], enabled);
            req.provider.protocol = Protocol::OpenAi;
            req.provider.base_url = "https://api.deepseek.com".into();
            assert_eq!(
                body(&req, &[], &[]).unwrap()["thinking"]["type"],
                if enabled { "enabled" } else { "disabled" }
            );
            req.provider.base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1".into();
            assert_eq!(body(&req, &[], &[]).unwrap()["enable_thinking"], enabled);
            req.provider.base_url = "https://openrouter.ai/api/v1".into();
            assert_eq!(
                body(&req, &[], &[]).unwrap()["reasoning"]["enabled"],
                enabled
            );
            req.provider.base_url = "https://api.openai.com/v1".into();
            req.conversation.model = "gpt-5.1".into();
            assert_eq!(
                body(&req, &[], &[]).unwrap()["reasoning_effort"],
                if enabled { "medium" } else { "none" }
            );
            req.provider.protocol = Protocol::Ollama;
        }
        req.provider.protocol = Protocol::OpenAi;
        req.conversation.model = "o3".into();
        assert!(
            body(&req, &[], &[]).is_err(),
            "Never silently claim to disable an always-thinking model"
        );
        req.provider.protocol = Protocol::Anthropic;
        req.settings.thinking = Some(true);
        req.conversation.model = "claude-sonnet-4-5".into();
        let value = body(&req, &[], &[]).unwrap();
        assert_eq!(value["thinking"]["budget_tokens"], 4096);
        assert!(value.get("temperature").is_none());
        req.settings.max_tokens = 1000;
        assert!(body(&req, &[], &[]).is_err());
    }
    #[test]
    fn claude_signed_thinking_survives_stream_and_tool_continuation() {
        let mut accumulator = Accumulator::default();
        for value in [
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Think carefully"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"signed-value"}}),
        ] {
            accumulator.ingest(&Protocol::Anthropic, value).unwrap();
        }
        let mut message = Message::new("assistant", "");
        accumulator.apply(&mut message);
        message.tools.push(ToolCall {
            id: "call".into(),
            name: "lookup".into(),
            arguments: "{}".into(),
            result: Some("found".into()),
        });
        let decoded: Message =
            serde_json::from_str(&serde_json::to_string(&message).unwrap()).unwrap();
        let history = messages(&Protocol::Anthropic, &[decoded], "").unwrap();
        assert_eq!(history[0]["content"][0]["thinking"], "Think carefully");
        assert_eq!(history[0]["content"][0]["signature"], "signed-value");
        assert_eq!(history[0]["content"][1]["type"], "tool_use");
    }
    #[test]
    fn trusted_bash_skips_confirmation_and_returns_real_output_to_model() {
        let (url, requests, handle) = server(vec![
            Response::sse(&[
                json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"trusted","function":{"name":"bash","arguments":"{\"command\":\"echo trusted-output\"}"}}]},"finish_reason":"tool_calls"}]}),
            ]),
            Response::sse(&[
                json!({"choices":[{"delta":{"content":"Finished"},"finish_reason":"stop"}]}),
            ]),
        ]);
        let mut req = request(Protocol::OpenAi, url);
        req.settings.tools_enabled = true;
        req.settings.bash_enabled = true;
        req.settings.bash_trusted = true;
        #[cfg(windows)]
        {
            req.settings.bash_path = "C:/Program Files/Git/bin/bash.exe".into();
        }
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || run(req, tx, Arc::new(AtomicBool::new(false))));
        loop {
            match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
                Event::Approval(_, _) => panic!("Trusted Bash must not ask for approval"),
                Event::Error(e) => panic!("{e}"),
                Event::Done => break,
                _ => {}
            }
        }
        let _ = requests.recv().unwrap();
        let (_, body) = requests.recv().unwrap();
        assert!(
            body["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap()
                .contains("trusted-output")
        );
        worker.join().unwrap();
        handle.join().unwrap();
    }
    #[test]
    fn truncated_stream_returns_error_instead_of_silent_success() {
        let (url, _requests, handle) = server(vec![Response {
            content_type: "text/event-stream",
            body: format!(
                "data: {}\n\n",
                json!({"choices":[{"delta":{"content":"partial"}}]})
            ),
            extra_headers: String::new(),
        }]);
        let (tx, rx) = mpsc::channel();
        run(
            request(Protocol::OpenAi, url),
            tx,
            Arc::new(AtomicBool::new(false)),
        );
        assert!(rx.try_iter().any(|e| matches!(e, Event::Error(_))));
        handle.join().unwrap();
    }
    #[test]
    fn openai_fragmented_tools_and_reasoning() {
        let mut a = Accumulator::default();
        a.ingest(&Protocol::OpenAi, json!({"choices":[{"delta":{"reasoning_content":"think","tool_calls":[{"index":0,"id":"c1","function":{"name":"bash","arguments":"{\"com"}}]}}]})).unwrap();
        a.ingest(&Protocol::OpenAi, json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"mand\":\"pwd\"}"}}]},"finish_reason":"tool_calls"}],"usage":{"completion_tokens":10}})).unwrap();
        assert_eq!(a.calls[&0].arguments, "{\"command\":\"pwd\"}");
        assert_eq!(a.reasoning, "think");
        assert!(a.finished);
    }
    #[test]
    fn tool_results_keep_protocol_order() {
        let mut m = Message::new("assistant", "");
        m.tools.push(ToolCall {
            id: "call_1".into(),
            name: "lookup".into(),
            arguments: "{}".into(),
            result: Some("found".into()),
        });
        let openai = messages(&Protocol::OpenAi, &[m.clone()], "system").unwrap();
        assert_eq!(openai[2]["tool_call_id"], "call_1");
        let claude = messages(&Protocol::Anthropic, &[m], "system").unwrap();
        assert_eq!(claude[1]["content"][0]["tool_use_id"], "call_1");
    }
    #[test]
    fn ollama_and_claude_streams() {
        let mut a = Accumulator::default();
        a.ingest(
            &Protocol::Ollama,
            json!({"message":{"content":"你好","thinking":"思考"},"done":true,"eval_count":2}),
        )
        .unwrap();
        assert_eq!(a.output, "你好");
        assert!(a.finished);
        let mut a = Accumulator::default();
        a.ingest(&Protocol::Anthropic, json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"t","name":"lookup"}})).unwrap();
        a.ingest(&Protocol::Anthropic, json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{}"}})).unwrap();
        assert_eq!(a.calls[&2].arguments, "{}");
    }
}
