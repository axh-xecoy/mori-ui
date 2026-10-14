use crate::model::*;
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub fn hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
pub struct McpClient {
    config: McpConfig,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    rx: Option<mpsc::Receiver<Value>>,
    http: Client,
    session: Option<String>,
    version: String,
    next: u64,
}
impl Drop for McpClient {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(session) = &self.session {
            let _ = self
                .http
                .delete(&self.config.url)
                .header("Mcp-Session-Id", session)
                .send();
        }
    }
}
impl McpClient {
    pub fn connect(config: McpConfig) -> Result<Self> {
        let mut client = Self {
            config,
            child: None,
            stdin: None,
            rx: None,
            http: Client::builder().timeout(Duration::from_secs(60)).build()?,
            session: None,
            version: "2025-06-18".into(),
            next: 0,
        };
        if client.config.url.is_empty() {
            let mut child = hidden_command(&client.config.command)
                .args(&client.config.args)
                .envs(&client.config.env)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .context("无法启动 MCP 服务")?;
            client.stdin = child.stdin.take();
            let stdout = child.stdout.take().context("MCP stdout 不可用")?;
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if let Ok(value) = serde_json::from_str(&line)
                        && tx.send(value).is_err()
                    {
                        break;
                    }
                }
            });
            client.rx = Some(rx);
            client.child = Some(child);
        }
        let init = client.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "mori-chat", "version": "0.1.0"}}))?;
        client.version = init["protocolVersion"]
            .as_str()
            .unwrap_or("2025-06-18")
            .into();
        client.send(json!({"jsonrpc":"2.0", "method":"notifications/initialized"}))?;
        Ok(client)
    }
    fn send(&mut self, value: Value) -> Result<Option<Value>> {
        if let Some(stdin) = &mut self.stdin {
            writeln!(stdin, "{}", value)?;
            stdin.flush()?;
            return Ok(None);
        }
        let mut req = self
            .http
            .post(&self.config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", &self.version)
            .json(&value);
        if let Some(session) = &self.session {
            req = req.header("Mcp-Session-Id", session);
        }
        let response = req.send()?.error_for_status()?;
        if let Some(session) = response.headers().get("mcp-session-id") {
            self.session = Some(session.to_str()?.into());
        }
        if response.status().as_u16() == 202 {
            return Ok(None);
        }
        let sse = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| s.starts_with("text/event-stream"));
        if !sse {
            return Ok(Some(response.json()?));
        }
        let mut data = String::new();
        for line in BufReader::new(response).lines() {
            let line = line?;
            if line.is_empty() && !data.is_empty() {
                let payload: Value = serde_json::from_str(data.trim())?;
                data.clear();
                if payload["id"] == value["id"]
                    && (payload.get("result").is_some() || payload.get("error").is_some())
                {
                    return Ok(Some(payload));
                }
            } else if let Some(part) = line.strip_prefix("data:") {
                data.push_str(part.trim_start());
                data.push('\n');
            }
        }
        bail!("MCP 事件流未返回响应")
    }
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next += 1;
        let id = self.next;
        let response =
            self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        let result = if let Some(response) = response {
            response
        } else {
            let rx = self.rx.as_ref().context("MCP 连接无响应")?;
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                let value = rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .context("MCP 请求超时")?;
                if value["id"] == id {
                    break value;
                }
                // Server-initiated sampling / elicitation are not granted.
                if value.get("method").is_some()
                    && value.get("id").is_some()
                    && let Some(stdin) = &mut self.stdin
                {
                    writeln!(
                        stdin,
                        "{}",
                        json!({"jsonrpc":"2.0","id":value["id"],"error":{"code":-32601,"message":"Client capability not supported"}})
                    )?;
                    stdin.flush()?;
                }
            }
        };
        if let Some(error) = result.get("error") {
            bail!("MCP: {}", error);
        }
        result.get("result").cloned().context("MCP 响应缺少 result")
    }
    pub fn list(&mut self) -> Result<Vec<Value>> {
        let mut tools = vec![];
        let mut cursor = None;
        for _ in 0..20 {
            let page = self.request(
                "tools/list",
                cursor
                    .as_ref()
                    .map_or(json!({}), |c: &String| json!({"cursor":c})),
            )?;
            if let Some(items) = page["tools"].as_array() {
                tools.extend(items.iter().cloned());
            }
            cursor = page["nextCursor"].as_str().map(str::to_string);
            if cursor.is_none() {
                return Ok(tools);
            }
        }
        bail!("MCP 工具列表分页过多")
    }
}

pub enum Target {
    Bash,
    Function(FunctionConfig),
    Mcp(usize, String),
}
pub struct ToolRegistry {
    pub schemas: Vec<Value>,
    pub targets: std::collections::HashMap<String, Target>,
    pub clients: Vec<McpClient>,
}
impl ToolRegistry {
    pub fn load(settings: &Settings) -> Result<Self> {
        let mut this = Self {
            schemas: vec![],
            targets: Default::default(),
            clients: vec![],
        };
        if !settings.tools_enabled {
            return Ok(this);
        }
        if settings.bash_enabled {
            this.add("bash".into(), if settings.bash_trusted { "执行一条 Bash 命令；用户已启用信任调用。" } else { "执行一条 Bash 命令；每次都需用户确认。" }.into(), json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}), Target::Bash)?;
        }
        for f in settings.functions.iter().filter(|f| f.enabled) {
            this.add(
                f.name.clone(),
                f.description.clone(),
                f.parameters.clone(),
                Target::Function(f.clone()),
            )?;
        }
        for (i, config) in settings.mcp.iter().filter(|c| c.enabled).enumerate() {
            let mut client = McpClient::connect(config.clone())
                .with_context(|| format!("MCP {} 连接失败", config.name))?;
            for (j, tool) in client.list()?.into_iter().enumerate() {
                let name = tool["name"]
                    .as_str()
                    .context("MCP 工具缺少名称")?
                    .to_string();
                this.add(
                    format!("mcp_{i}_{j}"),
                    format!(
                        "{} / {}: {}",
                        config.name,
                        name,
                        tool["description"].as_str().unwrap_or("")
                    ),
                    tool["inputSchema"].clone(),
                    Target::Mcp(i, name),
                )?;
            }
            this.clients.push(client);
        }
        Ok(this)
    }
    fn add(
        &mut self,
        name: String,
        description: String,
        parameters: Value,
        target: Target,
    ) -> Result<()> {
        if name.is_empty()
            || name.len() > 64
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            bail!("工具名需为 1–64 个英文字母、数字、下划线或连字符");
        }
        if this_duplicate(&self.targets, &name) {
            bail!("工具名称重复：{name}");
        }
        self.schemas.push(json!({"type":"function","function":{"name":name,"description":description,"parameters":parameters}}));
        self.targets.insert(name, target);
        Ok(())
    }
    pub fn is_bash(&self, name: &str) -> bool {
        matches!(self.targets.get(name), Some(Target::Bash))
    }
    pub fn execute(
        &mut self,
        call: &ToolCall,
        settings: &Settings,
        cancel: &AtomicBool,
    ) -> Result<String> {
        let args: Value = serde_json::from_str(&call.arguments).context("工具参数不是合法 JSON")?;
        match self.targets.get(&call.name).context("工具不存在或未启用")? {
            Target::Bash => {
                let command = args["command"].as_str().context("缺少 command")?;
                run_process(
                    &settings.bash_path,
                    &["-lc".into(), command.into()],
                    None,
                    cancel,
                )
            }
            Target::Function(f) => run_process(&f.command, &f.args, Some(&call.arguments), cancel),
            Target::Mcp(i, name) => Ok(self.clients[*i]
                .request("tools/call", json!({"name":name,"arguments":args}))?
                .to_string()),
        }
    }
}
fn this_duplicate(map: &std::collections::HashMap<String, Target>, name: &str) -> bool {
    map.contains_key(name)
}

fn run_process(
    program: &str,
    args: &[String],
    input: Option<&str>,
    cancel: &AtomicBool,
) -> Result<String> {
    let mut child = hidden_command(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take()
        && let Some(input) = input
    {
        stdin.write_all(input.as_bytes())?;
    }
    let stdout = child.stdout.take().context("stdout unavailable")?;
    let stderr = child.stderr.take().context("stderr unavailable")?;
    let read = |stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = vec![];
            let mut stream = stream;
            let mut chunk = [0u8; 8192];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if bytes.len() < 128 * 1024 {
                            bytes.extend_from_slice(&chunk[..n.min(128 * 1024 - bytes.len())]);
                        }
                    }
                }
            }
            String::from_utf8_lossy(&bytes).to_string()
        })
    };
    let out = read(Box::new(stdout));
    let err = read(Box::new(stderr));
    let start = Instant::now();
    let status = loop {
        if cancel.load(Ordering::Relaxed) || start.elapsed() > Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("执行已停止或超过 60 秒");
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    // Readers may remain open if the invoked program launches background children.
    for _ in 0..25 {
        if out.is_finished() && err.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if !out.is_finished() || !err.is_finished() {
        bail!("子进程持有输出管道，无法取得完整结果");
    }
    Ok(format!(
        "exit: {}\n{}\n{}",
        status,
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default()
    ))
}
