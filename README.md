# Mori

一个用 Rust 和 **gpui-kit 0.7.3** 编写的原生 AI 聊天客户端。暖白与墨绿的界面，以日常对话、图片、资料和个人助手为中心。

项目目录与 Cargo 包名为 `mori-ui`，界面品牌统一为 **Mori**。图标源文件是 `assets/mori.svg`，PNG 和多尺寸 Windows ICO 由 `scripts/build-icons.cjs` 生成（需要 Node.js 与 `sharp`）。图标内嵌到程序，不依赖运行目录；Windows 构建需要 SDK 的 `rc.exe` 或 MinGW 的 `windres`。现有会话数据目录和系统密钥标识保持兼容。

![Mori 首页](artifacts/screenshots/welcome.png)

## 运行

需要 Rust 稳定版与对应平台的原生构建工具。当前已在 Windows / Rust 1.99 上构建并验证。

```powershell
cargo run
```

编译后的 Windows 应用是 `target/debug/mori.exe`。发布构建使用 `cargo build --release`，产物在 `target/release/mori.exe`。如果旧应用仍在运行，先关闭旧窗口以保存会话，再启动新版。

第一次使用：打开左下角「设置」，选择服务商，填写 API 地址和密钥，点击「连接并获取模型」。本地 Ollama 的默认地址是 `http://localhost:11434`，不需要密钥。点击聊天页右上角的模型选择器开始使用。预设模型是便于开始的示例，模型发现和手动填写支持服务商实际提供的模型。

Ollama 原生协议填写服务器根地址（例如 `http://localhost:11434`）；尾部 `/v1` 或 `/api` 会自动规范化，兼容旧配置。若要使用 Ollama 的 OpenAI 兼容接口，选择 OpenAI 协议并保留 `/v1`。保存配置不代表服务器连接已验证，请使用「连接并获取模型」测试。

## 已实现

- **多服务商**：Ollama 原生协议、OpenAI Chat Completions 协议、Anthropic Messages 协议。预置 OpenAI、Claude、Gemini、DeepSeek、Qwen、SiliconFlow、OpenRouter、Groq、LM Studio；可添加自定义兼容接口。
- **聊天**：流式响应、Markdown、代码块、表格、可选择文本、思考过程折叠、回复耗时与输出 token 数、停止、重试、复制和朗读。
- **数学公式**：正文和思考过程支持 `$…$` 行内公式、`$$…$$` 独立公式、`\(...\)` / `\[...\]`。本地 RaTeX 排版支持常见分数、上下标、根号、希腊字母、矩阵与中文标注，随主题调整颜色。代码块保持原样，未闭合或解析失败时保留源码；会话保存、复制消息与导出仍保留原始 LaTeX。
- **会话管理**：搜索标题和正文、置顶、列表直接重命名和删除、撤销最近一次删除、草稿自动保存、编辑消息时保留原会话并创建分支、从任意回复分支、JSON 导入与 Markdown / JSON 导出。
- **多模态**：选择或拖入图片、UTF-8 文档和 PDF；图片消息预览。Ollama 接收图片与 PDF 提取的文字，云端支持原生图片 / PDF 内容块。单个附件最多 20 MB。
- **输入与设置**：输入框有独立文件拖拽区域；图片草稿显示缩略图，附件托盘可横向滚动。用户消息与头像在右侧，模型在左侧。侧栏右边缘可拖动调宽并自动保存。设置分为通用、模型连接、数据与隐私，支持简体中文 / English、浅色 / 深色和 Enter / Ctrl+Enter 发送（Shift+Enter 换行）。
- **模型管理**：发现的模型逐项显示，可手动添加、移除、设置显示名称和高级参数。模型 ID 用于请求，名称默认等于 ID；自定义名称显示在选择器、侧栏和回复头像旁，新回复保存实际模型 / 服务商标识。
- **图片与语音**：通过支持相应接口的 OpenAI 兼容服务生成图片、朗读消息、导入音频转写，以及麦克风录音转写。转写先放入输入框，发送由用户决定。
- **助手**：预设日常、写作、学习和翻译伙伴，自定义名称与系统提示词。
- **工具**：Function call、自定义函数程序、MCP stdio / Streamable HTTP、Bash。逐次展示工具参数，由用户允许或拒绝，然后把结果交回模型继续对话。
- **外观**：完整 SVG 图标资源、同步的绿色主题颜色、浅色 / 深色外观、统一卡片宽度与间距、小窗口布局。

## 本地数据与密钥

会话库与附件位于操作系统的应用数据目录。在 Windows 上通常为 `%LOCALAPPDATA%/mori/Mori/data`；以设置中的「打开数据目录」为准。`library.json` 通过临时文件替换保存，保留上一版 `library.backup.json`。加载损坏的会话库时禁用覆盖保存，原文件保留。

偏好独立保存到同目录的 `preferences.json`，优先于会话库中的设置。旧安装首次保存时自动沿用原设置；只更新当前实例修改过的偏好，保留未知偏好字段。保存通过文件锁与原子替换完成，旧版本退出时重写会话库不会重置已独立保存的偏好。

API Key 保存到系统凭据库，不写入会话 JSON。可用服务商 ID 对应的环境变量覆盖，例如 `OPENAI_API_KEY`、`ANTHROPIC_API_KEY`、`DEEPSEEK_API_KEY`、`QWEN_API_KEY`、`SILICONFLOW_API_KEY`、`OPENROUTER_API_KEY`。自定义服务商以设置中的内部 UUID 为 ID。

`MORI_DATA_DIR` 可以指定独立的数据目录，适合测试。JSON 导出包含配置与会话，但图片等二进制附件仍通过本地路径引用；完整迁移应同时复制数据目录。MCP 自定义环境变量保存在本地配置，导出时也会包含它们，因此不建议在其中放入长期凭据。

## 工具和 MCP

在「工具与扩展」里填写 MCP 的名字，以及远程地址或本地程序和启动参数。保存后开启工具，发送聊天消息时会初始化连接并发现工具。服务器认证可以通过本地进程的环境变量配置；HTTP OAuth、旧版 HTTP+SSE、MCP sampling / elicitation 尚未实现。

「高级设置与自定义函数」允许配置更细的选项。例如：

```json
{
  "tools_enabled": true,
  "bash_enabled": false,
  "mcp": [
    {
      "name": "资料库",
      "command": "npx.cmd",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "C:/Users/Public/Documents"],
      "env": {},
      "url": "",
      "enabled": true
    }
  ],
  "functions": []
}
```

未填写的设置保留当前值。高级编辑器不包含语言、发送方式、主题和侧栏宽度，避免扩展配置覆盖界面偏好。自定义函数的结构为 `name`、`description`、`parameters`（JSON Schema）、`command`、`args`、`enabled`。参数 JSON 通过 stdin 交给指定程序，stdout / stderr 返回模型。Bash 和函数程序不在沙箱中运行；实际执行前会显示参数并请求确认。每次生成最多 12 轮工具调用，函数 / Bash 单次执行最多 60 秒。

Bash 默认在系统桌面目录执行，支持 Windows 桌面重定向；每次调用独立启动，可在同一条命令中用 `cd` 切换目录。通过非登录 `bash -s --` 的 stdin 传入 UTF-8 脚本，避免 Windows Bash 的命令行参数编码破坏中文 heredoc 内容。模型会收到本机操作系统、实际桌面路径、Shell 和路径用法说明，不应假设存在 `/mnt/data` 等托管环境目录。桌面无法定位或不可访问时返回明确错误。长参数与输出各有独立滚动条，滚动到边界也不会带动聊天页面。

## 能力范围

在「设置 → 模型连接」点击模型行的参数按钮，可设置名称、Temperature、Top P、输出上限与更多 JSON 参数。输出留空不设置客户端上限，采样留空省略参数。自定义 JSON 支持 `top_k`、`seed`、`stop`、惩罚参数、`reasoning_effort`、`response_format` 与原生嵌套 `options`；字段是否支持及数值范围由服务商决定。`null` 删除请求中的该参数，嵌套对象合并；模型参数优先于全局参数。会话管理的 `model/messages/stream/tools/system` 不允许通过参数覆盖。全局 JSON 参数可在扩展高级设置的 `request_parameters` 中配置。

默认输出上限为 0：OpenAI 兼容请求省略上限字段，Ollama 使用 `num_predict: -1`。旧版本的默认 8192 在首次升级时迁移为 0，保留其他偏好；迁移后明确设置的 8192 仍保留。Claude 要求正整数 `max_tokens`，请按实际模型填写；服务商和模型自身的输出 / 上下文限制仍有效。参数适配依据：[OpenAI Chat Completions](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)、[Ollama Modelfile](https://docs.ollama.com/modelfile)、[Claude Messages](https://platform.claude.com/docs/en/api/messages/create)。

名称与参数同时保存在会话库和独立偏好中，防止旧版本重写会话库或过期实例退出时覆盖新配置；刷新模型列表不会清除其配置。旧回复没有独立模型标识时，以原会话的模型名称作为回退。

OpenAI 兼容连接可选择「自动识别 / 标准兼容 / llama.cpp」。自动识别依据 `/models` 返回的 `owned_by`，按服务商与 URL 缓存 10 分钟；首次请求探测最多等待 2 秒，探测失败沿用标准兼容适配，也可手动指定后端。llama.cpp 的开关写入 `chat_template_kwargs.enable_thinking`，显式开启 / 关闭优先于高级 JSON 中冲突的思考参数；自动模式仍沿用自定义值与服务端默认。该后端默认请求 `reasoning_format: deepseek`，将思考内容返回到独立字段供界面折叠，自定义 `reasoning_format` 可覆盖此默认值。依据：[llama.cpp 服务端文档](https://github.com/ggml-org/llama.cpp/tree/master/tools/server#post-v1chatcompletions-openai-compatible-chat-completions-api)。

图片、PDF、工具、图像生成和音频接口取决于所选服务商与模型。OpenAI 兼容服务商不一定提供图像和音频接口。音频输入是录音后转写，不是 Realtime 全双工语音；本地 PDF 文本提取不执行 OCR。当前没有实现账户云同步、向量知识库、原生网页搜索或 Responses API；可以通过 MCP 接入搜索与资料服务。

停止按钮会立即结束界面接收并保留当前回复。阻塞 HTTP 连接会在下一段响应到来或超时后释放，未实现服务商后台任务的远程取消。当前一次运行一个生成请求，支持生成中查看其他会话。

## 验证

当前验收结论与正式发行前的缺口见 [ACCEPTANCE.md](ACCEPTANCE.md)。GitHub Actions 在推送与 PR 时执行 Windows 构建、测试和代码检查。

```powershell
cargo test
cargo clippy --all-targets --features visual-test -- -D warnings
cargo fmt --check
```

测试使用本地 HTTP 模拟服务，覆盖三种服务商协议的完整流式请求、跨网络块的 UTF-8 / SSE、工具拒绝后的继续回复、异常断流、MCP 初始化 / 工具发现 / 执行 / 会话头、本地保存与备份。不会调用真实付费服务。

界面验证使用 GPUI 自身的 `render_to_image`，不是网页仿制效果图。运行下列命令将输出 59 个实际界面状态，并自动检查重命名、删除、恢复、保存服务商提示、数学公式、设置分页、英文、小窗口、图片附件预览，以及输入框的 Enter / Ctrl+Enter / Shift+Enter 动作分发、侧栏鼠标拖动和工具输出滚轮隔离：

```powershell
$env:MORI_DATA_DIR = "$PWD/.mori-visual"
$env:MORI_SNAPSHOT_DIR = "$PWD/artifacts/screenshots"
cargo run --features visual-test
Remove-Item Env:MORI_DATA_DIR
Remove-Item Env:MORI_SNAPSHOT_DIR
```

截图模式不保存模拟会话，不发起模型请求，不开启麦克风，同时检查后台附件导入切换会话时的归属。

## 代码结构

- `src/ui/`：原生页面、统一视觉样式、设置和渲染检查。
- `src/i18n.rs`：简体中文与 English 的界面文案。
- `src/ui.rs`：会话操作、草稿、异步任务、工具确认、附件与音频交互。
- `src/provider.rs`：请求转换、流式解析、工具对话循环、图片与音频 API。
- `src/tools.rs`：MCP 传输、工具注册、受用户确认的程序执行。
- `src/storage.rs`：会话保存、附件、PDF 文本提取与系统密钥。
- `src/audio.rs`：原生麦克风录音。
- `src/model.rs`：可序列化的数据模型。

框架：[GPUI Kit](https://github.com/longbridge/gpui-kit)。接口遵循 [Ollama](https://docs.ollama.com/api/chat)、[OpenAI](https://developers.openai.com/api/reference/resources/chat)、[Anthropic](https://platform.claude.com/docs/en/api/messages) 和 [MCP](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports) 文档。

### 思考与工具信任

输入框下方的思考菜单支持自动、开启和关闭；自动保持服务商默认。Ollama、DeepSeek、Qwen / SiliconFlow、OpenRouter、Claude 和部分 OpenAI 模型分别使用对应参数，强制思考的模型不能保证关闭。未适配的服务商会提示切回自动，不用隐藏思考过程冒充关闭。

在“工具与扩展”中启用 Bash 后，可选择信任调用，随后 Bash 命令无需逐次确认。默认关闭；MCP 与自定义函数仍需确认，命令在本机直接执行。流式显示约每 80ms 合并更新，状态区域预留高度，生成期间隐藏不停变化的耗时和 token 统计。

接口参考：[Ollama](https://github.com/ollama/ollama/blob/main/docs/openapi.yaml)、[Qwen](https://help.aliyun.com/zh/model-studio/deep-thinking/)、[DeepSeek](https://api-docs.deepseek.com/guides/thinking_mode/)、[OpenAI](https://developers.openai.com/api/docs/guides/reasoning)、[Claude](https://platform.claude.com/docs/en/build-with-claude/extended-thinking)、[OpenRouter](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)。

### 消息与长输出

工具调用默认折叠，展开后长返回值在限高区域滚动，超长预览明确提示并可复制完整已保存结果。用户消息操作位于气泡外侧下方。AI 代码块提供语言标识、复制、另存为与常见语言的浅 / 深色语法高亮，长代码块内部滚动；复制和下载只保留代码正文。

输入框支持粘贴图片和文件附件，图片直接预览，普通文字维持原生粘贴行为。附件导入完成前暂时禁用发送。回复状态固定在输入框上方；顶部提示自动淡出移出，新会话会检查继承模型是否仍可用。

长会话遇到明确的上下文超限错误时，会按完整轮次排除较早历史后自动重试，保留最近交互和最新请求。回复会显示调整说明，本地记录不删除。当前采用有提示的历史缩减，尚未实现语义摘要；最近一轮内容本身超过服务端窗口时，会提示增大服务端上下文或减少本轮内容。输出上限与上下文窗口是不同设置。

长回复性能：消息列表按可见范围虚拟化；流式正文 / 思考通过增量更新，结束时同步完整消息。未变化的正文、数学符号规范化及用户原文 HTML 使用有容量限制的共享文本缓存。超过 120 行或达到 8KB 的代码块使用只读代码视图，按可见行布局，保留高亮、选择、复制和下载；超长正文使用固定高度的分块滚动视图。所有优化只影响显示与更新，不截断模型回复或聊天存储。

压力检查可设置 `MORI_PERF_CHECK_ONLY=1` 后运行上述原生截图流程：覆盖 4,000 条消息、10,000 行代码、输入交互与嵌套滚动，并打印实际构建行数及耗时。开发构建与发布构建的耗时不能直接比较。
