use std::sync::atomic::{AtomicBool, Ordering};
static ENGLISH: AtomicBool = AtomicBool::new(false);
pub fn set_language(language: &str) {
    ENGLISH.store(language == "en", Ordering::Relaxed);
}
pub fn tr(text: &str) -> String {
    if !ENGLISH.load(Ordering::Relaxed) {
        return text.into();
    }
    for (prefix, english) in [
        ("模型设置无效：", "Invalid model settings: "),
        ("服务商返回 ", "Provider returned "),
        ("获取模型失败：", "Failed to fetch models: "),
        ("保存失败：", "Save failed: "),
        ("保存服务商失败：", "Failed to save provider: "),
        ("添加附件失败：", "Failed to add attachment: "),
        ("导入失败：", "Import failed: "),
        ("转写失败：", "Transcription failed: "),
        ("朗读失败：", "Speech failed: "),
        ("系统凭据库写入失败：", "Credential store error: "),
        ("本地存储不可用：", "Local storage unavailable: "),
        ("JSON 格式错误：", "Invalid JSON: "),
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return format!("{english}{rest}");
        }
    }
    match text {
        "新连接" => "New connection",
        "连接名称" => "Connection name",
        "管理连接" => "Manage connections",
        "添加你的第一个连接" => "Add your first connection",
        "选择接口协议，填写地址和密钥，再获取或手动添加模型。" => "Choose a protocol, enter the endpoint and API key, then fetch or add models.",
        "OpenAI 兼容" => "OpenAI compatible",
        "兼容后端" => "Compatible backend",
        "自动识别" => "Detect automatically",
        "标准兼容" => "Standard compatible",
        "模型 ID" => "Model ID",
        "模型与高级参数" => "Models and parameters",
        "添加模型" => "Add model",
        "模型高级参数" => "Model parameters",
        "移除模型" => "Remove model",
        "连接获取模型，或手动添加模型。" => "Fetch models from the provider or add one manually.",
        "显示名称（留空使用模型 ID）" => "Display name (defaults to model ID)",
        "输出上限（tokens）" => "Output limit (tokens)",
        "更多参数（JSON）" => "Additional parameters (JSON)",
        "保存模型设置" => "Save model settings",
        "模型设置已保存" => "Model settings saved",
        "返回连接设置" => "Back to connection settings",
        "服务商默认" => "Provider default",
        "默认使用模型 ID" => "Defaults to model ID",
        "不设置客户端上限" => "No client limit",
        "采样参数留空使用服务商默认；输出留空不设置客户端上限。Claude 必须填写输出上限。" => "Leave sampling blank for provider defaults; blank output sets no client limit. Claude requires an explicit output limit.",
        "支持 top_k、seed、stop、惩罚、reasoning_effort，以及 options 等服务商原生嵌套参数。null 表示不发送该参数。" => "Supports top_k, seed, stop, penalties, reasoning_effort and nested provider options. null omits a parameter.",
        "最长输出（0 = 不设置上限）" => "Output limit (0 = no client limit)",
        "剪贴板图片过大" => "Clipboard image is too large",

        "正在添加附件…" => "Adding attachments…",

        "当前模型已被移除或停用，请重新选择" => "This model was removed or disabled. Choose another model.",

        "粘贴图片.png" => "Pasted image.png", "剪贴板图片无法读取" => "Cannot decode clipboard image",

        "复制代码"=>"Copy code", "下载代码"=>"Download code", "保存代码"=>"Save code", "代码文件"=>"Code file", "代码已保存"=>"Code saved",

        "已返回" => "Returned", "调用中" => "Running", "未完成" => "Incomplete", "调用参数" => "Arguments", "返回结果" => "Output", "复制调用参数" => "Copy arguments", "复制完整已保存结果" => "Copy full saved output", "仅预览前 20,000 字符，可复制完整已保存结果" => "Preview limited to 20,000 characters; copy the full saved output",

        "会话操作" => "Conversation actions", "思考：自动" => "Thinking: Auto", "思考：开启" => "Thinking: On", "思考：关闭" => "Thinking: Off",
        "选择模型默认、开启或关闭思考；需要模型支持" => "Use model default, enable or disable thinking; model support required",
        "信任调用：开启" => "Trusted calls: On", "信任调用：关闭" => "Trusted calls: Off", "信任模式：自动执行 Bash 调用" => "Trusted mode: execute Bash automatically", "每次执行前确认" => "Confirm before each call", "●  Mori 正在回复…" => "●  Mori is responding…",

        "正在连接模型…"=>"Connecting to the model…", "正在回复…"=>"Receiving response…", "等待你的工具执行确认"=>"Waiting for your tool approval",
        "请填写名称和有效的 HTTP / HTTPS 地址"=>"Enter a name and a valid HTTP or HTTPS URL", "请先选择一个已启用的服务商"=>"Choose an enabled provider first", "先选择模型；本地服务可在设置中获取模型列表"=>"Choose a model first. Fetch local models in settings.",
        "先停止回复，再删除这条会话"=>"Stop the response before deleting this conversation", "已停止，已收到的内容已保存"=>"Stopped. The received content is saved.", "已创建会话分支，原对话保留"=>"Conversation branched. The original is preserved.",
        "正在生成图片…"=>"Generating an image…", "转写完成，可以插入输入框"=>"Transcription complete. You can insert it into the composer.", "附件已添加到原会话草稿"=>"Attachments added to the original conversation draft", "原会话已删除，附件未加入其他会话"=>"The original conversation was deleted. Attachments were not redirected.", "会话已导入；服务商和密钥沿用本机设置"=>"Conversations imported. Local provider and key settings are retained.",
        "正在转写录音…"=>"Transcribing recording…", "正在录音 · 点击麦克风结束（最多 2 分钟）"=>"Recording · Click the microphone to stop (up to 2 minutes)", "选择音频并转写…"=>"Choose audio to transcribe…", "正在准备朗读…"=>"Preparing speech…", "语音输入需要选择支持音频转写的 OpenAI 兼容服务商"=>"Voice input requires an OpenAI-compatible provider with transcription support",
        "请填写名称，以及远程地址或本地程序"=>"Enter a name and a remote URL or local program", "远程服务地址需使用 HTTP 或 HTTPS"=>"Remote URLs must use HTTP or HTTPS", "助手名称不能为空"=>"Assistant name cannot be empty", "创造性须为 0–2，最长输出须为正整数"=>"Creativity must be between 0 and 2; maximum output must be a positive integer", "温度须为 0–2，max_tokens 须大于 0"=>"Temperature must be between 0 and 2; max_tokens must be positive", "已移除系统凭据库中的密钥"=>"Saved key removed from the credential store",
        "连接已添加。开启工具后，聊天时会自动发现可用能力。"=>"Connection added. Enable tools to discover its capabilities during chat.",
        "帮我想几个适合周末的有趣活动，兼顾放松和新鲜感。"=>"Suggest a few fun weekend activities that feel both relaxing and refreshing.",
        "用一个生活中的例子，解释什么是第一性原理。"=>"Explain first principles thinking with an everyday example.",
        "我想写一封真诚自然的感谢信，请和我一起构思。"=>"Help me write a sincere, natural thank-you letter.",
        "请帮我把接下来发送的文字翻译成自然流畅的英文。"=>"Translate the text I send next into natural, fluent English.",
        "设置"=>"Settings", "通用"=>"General", "模型连接"=>"Model connections", "数据与隐私"=>"Data & privacy",
        "让 Mori 更合你的习惯。"=>"Make Mori feel like yours.", "外观、语言、输入方式与模型连接，都在这里。"=>"Appearance, language, messaging and model connections, all in one place.",
        "界面语言"=>"Interface language", "选择你熟悉的语言，聊天内容保持原样。"=>"Choose your language. Your conversations stay as written.",
        "外观"=>"Appearance", "柔和的颜色，适合白天与夜晚。"=>"Gentle colors for day and night.", "浅色"=>"Light", "深色"=>"Dark",
        "发送方式"=>"Send messages", "Shift+Enter 始终换行。"=>"Shift+Enter always starts a new line.",
        "侧栏宽度"=>"Sidebar width", "拖动侧栏右侧的分隔线调整，宽度会自动保存。"=>"Drag the divider at the sidebar's right edge. Your width is saved automatically.", "恢复默认宽度"=>"Reset width",
        "你的数据，留在你的设备上。"=>"Your data stays on your device.", "会话和附件保存在本地，API Key 保存在系统凭据库。"=>"Conversations and attachments stay local. API keys use the system credential store.",
        "本地会话库"=>"Local conversation library", "备份、导入与迁移"=>"Backup, import & migration", "JSON 导出包含会话和配置；完整备份还需要复制附件目录。"=>"JSON exports include conversations and settings. A full backup also needs the attachments directory.",
        "关于 Mori"=>"About Mori", "原生 AI 聊天客户端"=>"Native AI chat client", "服务商"=>"Providers", "选择服务商"=>"Choose a provider", "配置已自动保存"=>"Preferences saved automatically",
        "留一点空间给灵感"=>"A little space for big ideas", "新的对话"=>"New conversation", "搜索对话"=>"Search conversations",
        "对话"=>"Chat", "助手"=>"Assistants", "工具与扩展"=>"Tools & extensions", "你的对话"=>"Your conversations", "本地保存 · 你的私人空间"=>"Saved locally · Your private space",
        "你"=>"You", "日常助手"=>"Everyday assistant", "写作伙伴"=>"Writing partner", "学习伙伴"=>"Learning partner", "翻译助手"=>"Translator",
        "今天，让想法慢慢生长。"=>"Give your ideas room to grow.", "一个问题、一张图片，或一个还没成形的灵感。"=>"A question, a picture, or an idea still taking shape.",
        "捕捉灵感"=>"Find inspiration", "探索新知"=>"Explore ideas", "打磨文字"=>"Polish your writing", "跨越语言"=>"Cross languages",
        "一起想想，一件值得尝试的新事"=>"Find something new worth trying", "把复杂的概念，讲得简单一些"=>"Make complex ideas easier to understand", "让表达更清楚，也更像你"=>"Make your words clearer and more yours", "在不同语言里，找到合适的表达"=>"Find the right words across languages",
        "支持本地模型，也连接你喜欢的 AI 服务"=>"Local models and the AI services you love", "回答可能有误，重要信息请核实。"=>"AI can make mistakes. Check important information.",
        "发送"=>"Send", "停止"=>"Stop", "工具"=>"Tools", "工具已开启"=>"Tools enabled", "图片生成"=>"Generate image",
        "Ctrl+Enter 发送 · Enter 换行"=>"Ctrl+Enter to send · Enter for a new line", "Enter 发送 · Shift+Enter 换行"=>"Enter to send · Shift+Enter for a new line",
        "有什么想聊的？也可以附上图片或文档…"=>"What's on your mind? Add an image or document…", "添加图片或文档"=>"Add images or documents", "移除附件"=>"Remove attachment",
        "显示或隐藏侧栏"=>"Toggle sidebar", "显示侧栏"=>"Show sidebar", "选择模型"=>"Choose model", "选择一个模型"=>"Choose a model", "在设置中获取列表或输入模型 ID"=>"Fetch models or enter a model ID in settings", "管理服务商"=>"Manage providers",
        "复制消息"=>"Copy message", "导出为 Markdown"=>"Export as Markdown", "朗读（服务商需支持语音接口）"=>"Read aloud (requires audio support)", "重命名"=>"Rename", "重命名会话"=>"Rename conversation", "保存名称"=>"Save name", "关闭"=>"Close", "删除会话"=>"Delete conversation", "删除当前会话"=>"Delete current conversation", "置顶 / 取消置顶"=>"Pin / unpin", "撤销删除"=>"Undo delete", "关闭提示"=>"Dismiss notice", "重新生成"=>"Regenerate", "跳转到最新消息"=>"Jump to latest message",
        "查看思考过程"=>"Show reasoning", "收起思考过程"=>"Hide reasoning", "编辑并创建分支"=>"Edit and branch", "从这里创建分支"=>"Branch from here",
        "语音输入 / 结束录音"=>"Voice input / stop recording", "导入音频并转写"=>"Import and transcribe audio", "插入音频转写"=>"Insert transcription",
        "连接你喜欢的模型。"=>"Connect your favorite models.", "从本地 Ollama 到云端服务，一处管理。API Key 安全保存在系统凭据库。"=>"Local Ollama and cloud services, together. API keys are stored securely in your system credential store.",
        "服务商名称"=>"Provider name", "接口协议"=>"API protocol", "API 地址（含 /v1 等服务商前缀）"=>"API base URL (include provider prefixes such as /v1)", "API 地址（Ollama 原生：如 http://localhost:11434，无需 /v1）"=>"Ollama host (e.g. http://localhost:11434, without /v1)",
        "模型 ID（可手动填写，每行一个）"=>"Model IDs (one per line)", "每行一个模型 ID"=>"One model ID per line", "新的 API Key（留空保留现有密钥）"=>"New API key (leave blank to keep the saved key)",
        "保存配置"=>"Save configuration", "连接并获取模型"=>"Connect & fetch models", "已启用"=>"Enabled", "已停用"=>"Disabled", "添加兼容服务商"=>"Add compatible provider", "自定义服务商"=>"Custom provider", "移除已保存的密钥"=>"Remove saved key",
        "模型能力由服务商和所选模型决定；视觉、PDF、工具、图片生成与语音接口需对应支持。"=>"Vision, PDFs, tools, images and audio depend on the selected provider and model.", "你的空间"=>"Your space", "切换深色外观"=>"Switch to dark", "切换浅色外观"=>"Switch to light", "导出所有会话"=>"Export conversations", "导入会话"=>"Import conversations", "打开数据目录"=>"Open data folder",
        "为不同的想法，找一位伙伴。"=>"A companion for every kind of idea.", "助手只是不同的对话风格。你可以调整它的说明，或创建自己的伙伴。"=>"Assistants offer different conversation styles. Adjust their instructions or create your own.", "我的助手"=>"My assistants", "创建助手"=>"Create assistant", "助手名称"=>"Assistant name", "给它起个名字"=>"Give it a name", "给助手的说明（系统提示词）"=>"Assistant instructions (system prompt)", "保存助手"=>"Save assistant", "开始对话"=>"Start conversation", "由你定义的对话伙伴"=>"Your own conversation companion",
        "让对话，多一点可能。"=>"Give conversations more possibilities.", "连接外部资料和服务。工具由模型提出请求，每次执行前都由你确认。"=>"Connect resources and services. You approve every tool call before it runs.", "启用工具"=>"Enable tools", "启用 Bash"=>"Enable Bash", "已连接的服务"=>"Connected services", "添加服务"=>"Add service", "远程服务"=>"Remote service", "本地服务"=>"Local service", "本机命令"=>"Local command", "远程服务地址"=>"Remote server URL", "或者，启动本地程序"=>"Or start a local program", "启动参数（每行一个）"=>"Startup arguments (one per line)", "添加连接"=>"Add connection",
        "还没有连接服务。添加一个 MCP 服务，让助手访问你的资料与工具。"=>"No services connected yet. Add an MCP server to access your resources and tools.", "例如：我的资料库"=>"For example: My library", "例如：npx.cmd 或程序的完整路径"=>"For example: npx.cmd or a full executable path", "每行一个启动参数"=>"One startup argument per line", "https://example.com/mcp（本地程序留空）"=>"https://example.com/mcp (leave blank for local programs)",
        "检查参数后决定；工具可能读取文件、联网或修改本机内容。"=>"Review the arguments. Tools may read files, access the network or change local content.", "待确认"=>"Awaiting approval", "已处理"=>"Completed", "拒绝"=>"Reject", "允许这一次"=>"Allow once", "需要时才开启，每次执行前确认"=>"Enable when needed. Approve every execution.", "让助手查资料、使用你连接的服务"=>"Let assistants use your connected resources and services", "已启用，点击关闭"=>"Enabled · click to disable",
        "让回答更合心意"=>"Tune the responses", "创造性（0–2）"=>"Creativity (0–2)", "最长输出（tokens）"=>"Maximum output tokens", "图片生成模型"=>"Image generation model", "朗读模型"=>"Speech model", "语音转写模型"=>"Transcription model", "保存偏好"=>"Save preferences", "高级设置与自定义函数"=>"Advanced settings & custom functions", "收起高级设置"=>"Hide advanced settings", "验证并保存"=>"Validate & save", "可配置 Bash 路径、MCP 环境变量与自定义函数。"=>"Configure the Bash path, MCP environment variables and custom functions.", "函数参数 JSON 通过 stdin 交给 command，程序输出返回模型。每次调用都需要确认。"=>"Function arguments are sent as JSON on stdin. Program output returns to the model. Every call requires approval.",
        "支持 MCP stdio 和 Streamable HTTP"=>"Supports MCP stdio and Streamable HTTP", "连接外部能力"=>"Connect external capabilities", "●  Mori 正在思考…"=>"●  Mori is thinking…",
        "服务商配置已保存；尚未测试连接"=>"Provider configuration saved; connection not yet tested", "配置已保存，Ollama 原生协议地址已移除 /v1 或 /api 后缀；尚未测试连接"=>"Configuration saved. Ollama URL normalized; connection not yet tested", "正在获取模型列表…"=>"Fetching models…", "连接成功，模型列表已更新"=>"Connected. Model list updated", "会话已删除。可以撤销这次操作。"=>"Conversation deleted. You can undo this action.", "会话已恢复"=>"Conversation restored", "正在执行工具…"=>"Running tool…", "偏好已保存"=>"Preferences saved", "助手已保存"=>"Assistant saved", "扩展设置已保存"=>"Extension settings saved",
        _ => text,
    }.into()
}
