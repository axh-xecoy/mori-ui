use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub fn id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum Protocol {
    OpenAi,
    Ollama,
    Anthropic,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub models: Vec<String>,
    pub enabled: bool,
}
impl Provider {
    fn preset(name: &str, protocol: Protocol, url: &str, models: &[&str]) -> Self {
        Self {
            id: name.to_lowercase().replace(' ', "-"),
            name: name.into(),
            protocol,
            base_url: url.into(),
            models: models.iter().map(|s| s.to_string()).collect(),
            enabled: true,
        }
    }
    pub fn presets() -> Vec<Self> {
        vec![
            Self::preset("Ollama", Protocol::Ollama, "http://localhost:11434", &[]),
            Self::preset(
                "OpenAI",
                Protocol::OpenAi,
                "https://api.openai.com/v1",
                &["gpt-4.1", "gpt-4.1-mini", "o3", "gpt-4o"],
            ),
            Self::preset(
                "Anthropic",
                Protocol::Anthropic,
                "https://api.anthropic.com/v1",
                &["claude-sonnet-4-5", "claude-opus-4-1"],
            ),
            Self::preset(
                "Gemini",
                Protocol::OpenAi,
                "https://generativelanguage.googleapis.com/v1beta/openai",
                &["gemini-2.5-pro", "gemini-2.5-flash"],
            ),
            Self::preset(
                "DeepSeek",
                Protocol::OpenAi,
                "https://api.deepseek.com",
                &["deepseek-chat", "deepseek-reasoner"],
            ),
            Self::preset(
                "Qwen",
                Protocol::OpenAi,
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                &["qwen-plus", "qwen-turbo", "qwen-vl-max"],
            ),
            Self::preset(
                "SiliconFlow",
                Protocol::OpenAi,
                "https://api.siliconflow.cn/v1",
                &["Qwen/Qwen3-8B"],
            ),
            Self::preset(
                "OpenRouter",
                Protocol::OpenAi,
                "https://openrouter.ai/api/v1",
                &[],
            ),
            Self::preset(
                "Groq",
                Protocol::OpenAi,
                "https://api.groq.com/openai/v1",
                &[],
            ),
            Self::preset(
                "LM Studio",
                Protocol::OpenAi,
                "http://localhost:1234/v1",
                &[],
            ),
        ]
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Attachment {
    pub name: String,
    pub path: String,
    pub mime: String,
    pub text: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub reasoning: String,
    #[serde(default)]
    pub thinking_blocks: Vec<Value>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub tools: Vec<ToolCall>,
    #[serde(default)]
    pub tokens: u64,
    #[serde(default)]
    pub elapsed_ms: u64,
    #[serde(default)]
    pub error: Option<String>,
}
impl Message {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        Self {
            id: id(),
            role: role.into(),
            content: content.into(),
            reasoning: String::new(),
            thinking_blocks: vec![],
            attachments: vec![],
            tools: vec![],
            tokens: 0,
            elapsed_ms: 0,
            error: None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub provider_id: String,
    pub model: String,
    pub assistant_id: String,
    pub messages: Vec<Message>,
    pub pinned: bool,
    pub updated: DateTime<Local>,
    #[serde(default)]
    pub draft: String,
    #[serde(default)]
    pub draft_attachments: Vec<Attachment>,
}
impl Conversation {
    pub fn new(provider_id: String, model: String, assistant_id: String) -> Self {
        Self {
            id: id(),
            title: "新的对话".into(),
            provider_id,
            model,
            assistant_id,
            messages: vec![],
            pinned: false,
            updated: Local::now(),
            draft: String::new(),
            draft_attachments: vec![],
        }
    }
    pub fn markdown(&self) -> String {
        let mut out = format!(
            "# {}\n\n{} · {}\n\n",
            self.title,
            self.model,
            self.updated.format("%Y-%m-%d %H:%M")
        );
        for m in &self.messages {
            out.push_str(&format!(
                "## {}\n\n{}\n\n",
                if m.role == "user" { "你" } else { "助手" },
                m.content
            ));
            for a in &m.attachments {
                out.push_str(&format!("附件：{}\n\n", a.name));
            }
            for t in &m.tools {
                out.push_str(&format!(
                    "工具：{}\n```json\n{}\n```\n\n{}\n\n",
                    t.name,
                    t.arguments,
                    t.result.as_deref().unwrap_or("未执行")
                ));
            }
        }
        out
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Assistant {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
}
impl Assistant {
    pub fn presets() -> Vec<Self> {
        [("general", "日常助手", "解答疑问，整理思路", "你是一位细致、友善的助手。以用户使用的语言回答，保持准确、清晰。"),
        ("write", "文字伙伴", "打磨表达，让灵感成文", "你是一位写作伙伴，帮助用户写作、修改和润色。尊重原文风格，给出具体建议。"),
        ("learn", "学习搭子", "拆解知识，一起探索", "你是一位耐心的学习伙伴。用具体例子拆解概念，鼓励主动思考，循序渐进。"),
        ("translate", "翻译助手", "自然、准确的跨语言表达", "你是一位专业翻译。保留原文含义和语气，输出自然的目标语言；未指定目标语言时，中英文互译。")]
        .into_iter().map(|(id,name,description,prompt)| Self { id: id.into(), name: name.into(), description: description.into(), prompt: prompt.into() }).collect()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct McpConfig {
    pub name: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub enabled: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FunctionConfig {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Settings {
    pub language: String,
    pub send_on_enter: bool,
    pub sidebar_width: f32,
    pub temperature: f64,
    pub max_tokens: u32,
    pub dark: bool,
    pub tools_enabled: bool,
    pub bash_enabled: bool,
    pub bash_path: String,
    pub bash_trusted: bool,
    pub thinking: Option<bool>,
    pub mcp: Vec<McpConfig>,
    pub functions: Vec<FunctionConfig>,
    pub image_model: String,
    pub speech_model: String,
    pub transcription_model: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "zh-CN".into(),
            send_on_enter: false,
            sidebar_width: 252.,
            temperature: 0.7,
            max_tokens: 8192,
            dark: false,
            tools_enabled: false,
            bash_enabled: false,
            bash_path: "bash".into(),
            bash_trusted: false,
            thinking: None,
            mcp: vec![],
            functions: vec![],
            image_model: "gpt-image-1".into(),
            speech_model: "tts-1".into(),
            transcription_model: "whisper-1".into(),
        }
    }
}
impl Settings {
    pub fn patched(&self, json: &str) -> anyhow::Result<Self> {
        let patch: serde_json::Value = serde_json::from_str(json)?;
        let fields = patch
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("设置必须是 JSON 对象"))?;
        let mut merged = serde_json::to_value(self)?;
        merged.as_object_mut().unwrap().extend(fields.clone());
        Ok(serde_json::from_value(merged)?)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Library {
    pub providers: Vec<Provider>,
    pub conversations: Vec<Conversation>,
    pub assistants: Vec<Assistant>,
    pub settings: Settings,
    pub active_id: Option<String>,
}
impl Library {
    pub fn model_available(&self, provider_id: &str, model: &str) -> bool {
        !model.is_empty()
            && self
                .providers
                .iter()
                .any(|p| p.id == provider_id && p.enabled && p.models.iter().any(|m| m == model))
    }
    pub fn preferred_model(
        &self,
        current: Option<&Conversation>,
        selected: usize,
    ) -> (String, String) {
        if let Some(c) = current
            && self.model_available(&c.provider_id, &c.model)
        {
            return (c.provider_id.clone(), c.model.clone());
        }
        let preferred = current.and_then(|c| self.providers.iter().find(|p| p.id == c.provider_id));
        let mut candidates = preferred
            .into_iter()
            .chain(self.providers.get(selected))
            .chain(self.providers.iter());
        for p in candidates.clone().filter(|p| p.enabled) {
            if let Some(model) = p.models.iter().find(|m| !m.trim().is_empty()) {
                return (p.id.clone(), model.clone());
            }
        }
        candidates
            .find(|p| p.enabled)
            .map(|p| (p.id.clone(), String::new()))
            .unwrap_or_default()
    }
}
impl Default for Library {
    fn default() -> Self {
        Self {
            providers: Provider::presets(),
            conversations: vec![],
            assistants: Assistant::presets(),
            settings: Settings::default(),
            active_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_chats_do_not_inherit_removed_or_disabled_models() {
        let mut library = Library::default();
        let p = &mut library.providers[0];
        p.models = vec!["available".into()];
        let pid = p.id.clone();
        let mut current = Conversation::new(pid.clone(), "removed".into(), "general".into());
        assert_eq!(
            library.preferred_model(Some(&current), 0),
            (pid.clone(), "available".into())
        );
        current.model = "available".into();
        assert_eq!(
            library.preferred_model(Some(&current), 0),
            (pid.clone(), "available".into())
        );
        library.providers[0].enabled = false;
        let chosen = library.preferred_model(Some(&current), 0);
        assert_ne!(chosen.0, pid);
        assert!(library.model_available(&chosen.0, &chosen.1));
        library.providers.clear();
        assert_eq!(
            library.preferred_model(Some(&current), 99),
            (String::new(), String::new())
        );
        let mut library = Library::default();
        for provider in &mut library.providers {
            provider.models.clear();
        }
        assert!(library.preferred_model(Some(&current), 99).1.is_empty());
    }
    #[test]
    fn older_preferences_keep_their_values_and_gain_safe_defaults() {
        let settings: Settings =
            serde_json::from_value(serde_json::json!({"dark":true,"temperature":0.5})).unwrap();
        assert!(settings.dark);
        assert_eq!(settings.temperature, 0.5);
        assert_eq!(settings.language, "zh-CN");
        assert!(!settings.send_on_enter);
        assert_eq!(settings.sidebar_width, 252.);
        assert_eq!(settings.thinking, None);
        assert!(!settings.bash_trusted);
    }
}
