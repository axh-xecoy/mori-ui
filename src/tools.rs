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
    pub notices: Vec<String>,
    shell: Option<Settings>,
    pub targets: std::collections::HashMap<String, Target>,
    pub clients: Vec<McpClient>,
}
impl ToolRegistry {
    pub fn load(settings: &Settings) -> Result<Self> {
        let mut this = Self {
            schemas: vec![],
            notices: vec![],
            shell: None,
            targets: Default::default(),
            clients: vec![],
        };
        if !settings.tools_enabled {
            return Ok(this);
        }
        if settings.bash_enabled {
            match resolve_shell(settings) {
                Ok(shell) => {
                    let name = if shell.shell_kind == "bash" { "bash" } else { "shell" };
                    this.add(name.into(), bash_description(&shell)?, json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}), Target::Bash)?;
                    this.shell = Some(shell);
                }
                Err(error) => this.notices.push(format!("本机 Shell 不可用，已跳过命令工具：{error:#}。聊天与其他工具仍可使用，请在设置中配置 Shell。")),
            }
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
    pub fn is_local_shell(&self, name: &str) -> bool {
        matches!(self.targets.get(name), Some(Target::Bash))
    }
    pub fn execute(
        &mut self,
        call: &ToolCall,
        _settings: &Settings,
        cancel: &AtomicBool,
    ) -> Result<String> {
        let args: Value = serde_json::from_str(&call.arguments).context("工具参数不是合法 JSON")?;
        match self.targets.get(&call.name).context("工具不存在或未启用")? {
            Target::Bash => {
                let command = args["command"].as_str().context("缺少 command")?;
                let shell = self.shell.as_ref().context("Shell 尚未初始化")?;
                run_shell(shell, command, cancel)
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

fn decode_output(bytes: &[u8]) -> String {
    // PowerShell and native Windows programs can emit UTF-16 instead of UTF-8.
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little_endian = bytes[0] == 0xff;
        let units: Vec<_> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                if little_endian {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                }
            })
            .collect();
        let mut text = String::from_utf16_lossy(&units);
        if !bytes.len().is_multiple_of(2) {
            text.push('\u{fffd}');
        }
        return text;
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    #[cfg(windows)]
    {
        // Git Bash can emit a Windows-localized timezone in the ANSI code page,
        // even when LANG is UTF-8. Never replace those bytes before conversion.
        // SAFETY: These functions have no pointer arguments or preconditions.
        let code_pages = unsafe { [GetACP(), GetOEMCP()] };
        for code_page in code_pages {
            if let Some(text) = decode_windows_output(bytes, code_page) {
                return text;
            }
        }
    }
    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetACP() -> u32;
    fn GetOEMCP() -> u32;
    fn MultiByteToWideChar(
        code_page: u32,
        flags: u32,
        input: *const u8,
        input_len: i32,
        output: *mut u16,
        output_len: i32,
    ) -> i32;
}

#[cfg(windows)]
fn decode_windows_output(bytes: &[u8], code_page: u32) -> Option<String> {
    if bytes.is_empty() {
        return Some(String::new());
    }
    let input_len = i32::try_from(bytes.len()).ok()?;
    // SAFETY: Input remains valid for input_len bytes. A null output requests
    // its size; the second call receives a buffer with exactly that capacity.
    unsafe {
        let count = MultiByteToWideChar(
            code_page,
            8, /* MB_ERR_INVALID_CHARS */
            bytes.as_ptr(),
            input_len,
            std::ptr::null_mut(),
            0,
        );
        if count <= 0 {
            return None;
        }
        let mut output = vec![0u16; count as usize];
        let written = MultiByteToWideChar(
            code_page,
            8,
            bytes.as_ptr(),
            input_len,
            output.as_mut_ptr(),
            count,
        );
        if written <= 0 {
            return None;
        }
        output.truncate(written as usize);
        String::from_utf16(&output).ok()
    }
}

fn run_process(
    program: &str,
    args: &[String],
    input: Option<&str>,
    cancel: &AtomicBool,
) -> Result<String> {
    run_process_in(program, args, input, cancel, None)
}

pub fn bash_directory(settings: &Settings) -> Result<std::path::PathBuf> {
    let dirs = directories::UserDirs::new().context("无法定位用户主目录")?;
    let value = settings.bash_working_directory.trim();
    let path = if value.is_empty() || value == "~" {
        dirs.home_dir().to_path_buf()
    } else if value.starts_with("~/") || value.starts_with("~\\") {
        dirs.home_dir().join(&value[2..])
    } else {
        std::path::PathBuf::from(value)
    };
    anyhow::ensure!(path.is_absolute(), "工作目录必须是绝对路径或以 ~/ 开头");
    anyhow::ensure!(
        path.is_dir(),
        "工作目录不存在或不可访问：{}",
        path.display()
    );
    Ok(path)
}

fn inferred_shell(program: &str) -> Option<&'static str> {
    let normalized = program.replace('\\', "/");
    let name = normalized.rsplit('/').next()?.to_ascii_lowercase();
    match name.trim_end_matches(".exe") {
        "bash" => Some("bash"),
        "sh" | "dash" | "zsh" => Some("sh"),
        "pwsh" | "powershell" => Some("powershell"),
        "cmd" => Some("cmd"),
        _ => None,
    }
}

pub fn resolve_shell(settings: &Settings) -> Result<Settings> {
    bash_directory(settings)?;
    let automatic = settings.shell_kind == "auto";
    let configured = settings.bash_path.trim();
    // The legacy default 'bash' means automatic discovery after upgrading.
    let candidates: Vec<(&str, &str)> =
        if automatic && (configured.is_empty() || configured == "bash") {
            if cfg!(windows) {
                vec![
                    ("powershell", "pwsh"),
                    ("powershell", "powershell.exe"),
                    ("cmd", "cmd.exe"),
                    ("bash", "bash"),
                ]
            } else {
                vec![("bash", "bash"), ("sh", "/bin/sh")]
            }
        } else {
            let kind = if automatic {
                inferred_shell(configured).context("无法识别程序类型，请手动选择 Shell 类型")?
            } else {
                settings.shell_kind.as_str()
            };
            anyhow::ensure!(
                ["bash", "sh", "powershell", "cmd"].contains(&kind),
                "不支持的 Shell 类型"
            );
            if let Some(inferred) = inferred_shell(configured) {
                anyhow::ensure!(inferred == kind, "程序路径与选中的 Shell 类型不匹配");
            }
            vec![(kind, configured)]
        };
    let mut errors = Vec::new();
    for (kind, program) in candidates {
        let mut shell = settings.clone();
        shell.shell_kind = kind.into();
        shell.bash_path = program.into();
        match probe_shell(&shell) {
            Ok(()) => return Ok(shell),
            Err(error) => errors.push(format!("{program}: {error:#}")),
        }
    }
    bail!("{}", errors.join("；"))
}

fn probe_shell(settings: &Settings) -> Result<()> {
    let args: Vec<String> = match settings.shell_kind.as_str() {
        "bash" => vec!["--noprofile", "--norc", "-c", "test -n \"$BASH_VERSION\""],
        "sh" => vec!["-c", "__mori_shell=1; test \"$__mori_shell\" = 1"],
        "powershell" => vec![
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "if ($PSVersionTable) { exit 0 } else { exit 1 }",
        ],
        "cmd" => vec!["/D", "/C", "exit 0"],
        _ => bail!("不支持的 Shell 类型"),
    }
    .into_iter()
    .map(str::to_string)
    .collect();
    let mut child = hidden_command(&settings.bash_path)
        .args(args)
        .current_dir(bash_directory(settings)?)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("无法启动 {}", settings.bash_path))?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            anyhow::ensure!(status.success(), "Shell 类型验证失败（{status}）");
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(3) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Shell 启动检测超时");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn run_shell(settings: &Settings, script: &str, cancel: &AtomicBool) -> Result<String> {
    let directory = bash_directory(settings)?;
    match settings.shell_kind.as_str() {
        "bash" => run_bash(&settings.bash_path, script, cancel, &directory),
        "sh" => run_process_in(
            &settings.bash_path,
            &["-s".into()],
            Some(script),
            cancel,
            Some(&directory),
        ),
        "powershell" => {
            use base64::Engine as _;
            let script = format!(
                "$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); $OutputEncoding=[Console]::OutputEncoding; $PSDefaultParameterValues['Out-File:Encoding']='utf8'; $PSDefaultParameterValues['Set-Content:Encoding']='utf8'; $PSDefaultParameterValues['Add-Content:Encoding']='utf8';\n{script}\n"
            );
            // Keep the encoded bootstrap short: model-written scripts can exceed
            // Windows' command-line limit. Deliver the actual UTF-8 script on stdin.
            let bootstrap = "& ([scriptblock]::Create([System.IO.StreamReader]::new([Console]::OpenStandardInput(), [System.Text.UTF8Encoding]::new($false)).ReadToEnd()))";
            let bytes: Vec<u8> = bootstrap
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            run_process_in(
                &settings.bash_path,
                &[
                    "-NoLogo".into(),
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-EncodedCommand".into(),
                    encoded,
                ],
                Some(&script),
                cancel,
                Some(&directory),
            )
        }
        "cmd" => {
            let file = std::env::temp_dir().join(format!("mori-shell-{}.cmd", crate::model::id()));
            std::fs::write(
                &file,
                format!(
                    "@echo off\r\nchcp 65001 >nul\r\n{}\r\n",
                    script.replace("\r\n", "\n").replace('\n', "\r\n")
                ),
            )?;
            let result = run_process_in(
                &settings.bash_path,
                &[
                    "/D".into(),
                    "/Q".into(),
                    "/C".into(),
                    format!("call \"{}\"", file.display()),
                ],
                None,
                cancel,
                Some(&directory),
            );
            let _ = std::fs::remove_file(file);
            result
        }
        _ => bail!("不支持的 Shell 类型"),
    }
}

fn bash_description(settings: &Settings) -> Result<String> {
    let directory = bash_directory(settings)?
        .to_string_lossy()
        .replace('\\', "/");
    let platform = if cfg!(windows) {
        "Windows host: use Windows file paths appropriate to the selected Shell, or relative paths. Only when using Bash, inspect Git Bash/MSYS/WSL path mappings. Do not assume Linux directories or apt exist."
    } else if cfg!(target_os = "macos") {
        "macOS host: use macOS paths and the selected Shell syntax. Do not assume GNU-only flags or Linux package managers."
    } else {
        "Linux host: use Linux paths and the selected Shell syntax; do not assume Windows drive paths exist."
    };
    let syntax = match settings.shell_kind.as_str() {
        "powershell" => {
            "Use ONLY PowerShell syntax: Get-ChildItem, Set-Content -Encoding utf8, Join-Path, $env:NAME, and PowerShell here-strings. Never use Bash heredocs, export, or Unix command assumptions. Windows PowerShell 5.1 does not support &&. Use explicit UTF-8 for file writes."
        }
        "cmd" => {
            "Use ONLY Windows cmd.exe batch syntax: dir, type, copy, %NAME%, and Windows paths. Bash heredocs, export, PowerShell cmdlets and $variables are invalid. Scripts use UTF-8 code page 65001."
        }
        "sh" => "Use POSIX sh syntax only; avoid Bash-only features such as arrays and [[ ]].",
        _ => {
            "Use Bash syntax, UTF-8 file writes and quoted heredoc delimiters for literal content."
        }
    };
    Ok(format!(
        "Shell type: {}. {} Execute commands on the user's local machine. OS: {} ({}). Shell executable: {}. Initial working directory on EVERY call: {}. Each call starts a fresh shell; cd does not persist between calls. NOT a hosted Linux sandbox: never assume /mnt/data or /home/oai/share exists. {} Check available tools and directories first. Write text files as UTF-8 using the syntax of the selected Shell. {}",
        settings.shell_kind,
        syntax,
        std::env::consts::OS,
        std::env::consts::ARCH,
        settings.bash_path,
        directory,
        platform,
        if settings.bash_trusted {
            "The user enabled trusted execution."
        } else {
            "Each execution requires user confirmation."
        }
    ))
}

fn run_bash(
    program: &str,
    script: &str,
    cancel: &AtomicBool,
    directory: &std::path::Path,
) -> Result<String> {
    // Windows ports may decode argv using the ANSI codepage. Feed script bytes
    // directly instead, so heredocs and redirections preserve Chinese and emoji.
    let script = format!(
        "export LANG=C.UTF-8 LC_ALL=C.UTF-8\n{}\n",
        script.replace("\r\n", "\n")
    );
    run_process_in(
        program,
        &["-s".into(), "--".into()],
        Some(&script),
        cancel,
        Some(directory),
    )
}

fn run_process_in(
    program: &str,
    args: &[String],
    input: Option<&str>,
    cancel: &AtomicBool,
    directory: Option<&std::path::Path>,
) -> Result<String> {
    let mut command = hidden_command(program);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    #[cfg(windows)]
    if inferred_shell(program) == Some("cmd")
        && args.len() >= 2
        && args[args.len() - 2].eq_ignore_ascii_case("/C")
    {
        // cmd.exe parses its own command line. Rust's normal argv escaping
        // turns embedded quotes into literal backslash-quotes for this parser.
        use std::os::windows::process::CommandExt as _;
        command
            .args(&args[..args.len() - 1])
            .raw_arg(&args[args.len() - 1]);
    } else {
        command.args(args);
    }
    #[cfg(not(windows))]
    command.args(args);
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // Read output concurrently with input: a long heredoc can otherwise fill
    // stdin while an earlier command blocks on a full stdout pipe.
    let input = input.map(str::as_bytes).map(<[u8]>::to_vec);
    let stdin = child.stdin.take();
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        if let (Some(mut stdin), Some(input)) = (stdin, input) {
            stdin.write_all(&input)?;
        }
        Ok(())
    });
    let stdout = child.stdout.take().context("stdout unavailable")?;
    let stderr = child.stderr.take().context("stderr unavailable")?;
    let read = |stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = vec![];
            let mut truncated = false;
            let mut stream = stream;
            let mut chunk = [0u8; 8192];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let keep = n.min(128 * 1024 - bytes.len());
                        truncated |= keep < n;
                        bytes.extend_from_slice(&chunk[..keep]);
                    }
                }
            }
            // The byte limit may split a UTF-8 character. Keep its valid prefix
            // rather than misclassifying the entire stream as a legacy encoding.
            if truncated
                && let Err(error) = std::str::from_utf8(&bytes)
                && error.error_len().is_none()
            {
                bytes.truncate(error.valid_up_to());
            }
            let mut output = decode_output(&bytes);
            if truncated {
                output.push_str("\n[输出已截断 / output truncated]");
            }
            output
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
        if out.is_finished() && err.is_finished() && writer.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if !out.is_finished() || !err.is_finished() {
        bail!("子进程持有输出管道，无法取得完整结果");
    }
    if writer.is_finished() {
        let written = writer
            .join()
            .map_err(|_| anyhow::anyhow!("脚本输入线程异常"))?;
        // An explicit exit may close stdin before the remaining script is read.
        // Preserve that command's real exit status and captured diagnostics.
        if let Err(error) = written
            && error.kind() != std::io::ErrorKind::BrokenPipe
        {
            return Err(error).context("无法传入脚本");
        }
    } else {
        bail!("子进程持有输入管道，无法完成脚本传入");
    }
    Ok(format!(
        "exit: {}\n{}\n{}",
        status,
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default()
    ))
}

#[cfg(test)]
mod output_tests {
    use super::*;

    #[test]
    fn bash_utf8_heredoc_writes_exact_file_bytes() {
        let root = std::env::temp_dir().join(format!("mori-utf8-{}", crate::model::id()));
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(windows)]
        let shells = [
            "C:/Program Files/Git/bin/bash.exe",
            "C:/DevKit/w64devkit/bin/bash.exe",
        ];
        #[cfg(not(windows))]
        let shells = ["bash"];
        let body = format!(
            "<!doctype html>\n<title>可爱贪吃蛇 🐍</title>\n{}",
            "你好，世界！🌿\n".repeat(12000)
        );
        for shell in shells {
            #[cfg(windows)]
            if !std::path::Path::new(shell).is_file() {
                continue;
            }
            let script = format!(
                "printf '可爱贪吃蛇 🐍\\n'\nprintf '%160000s\\n' ''\ncat > './game.html' <<'MORI_EOF'\n{body}MORI_EOF\ncat './game.html' | head -n 2"
            );
            let output = run_bash(shell, &script, &AtomicBool::new(false), &root).unwrap();
            assert!(output.contains("可爱贪吃蛇 🐍"), "{shell}: {output}");
            assert_eq!(
                std::fs::read(root.join("game.html")).unwrap(),
                body.as_bytes(),
                "{shell}"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_bash_does_not_disable_other_tools() {
        let settings = Settings {
            tools_enabled: true,
            bash_enabled: true,
            bash_path: "mori-nonexistent-bash-executable".into(),
            ..Settings::default()
        };
        let mut registry = ToolRegistry::load(&settings).unwrap();
        assert!(!registry.is_local_shell("bash"));
        assert_eq!(registry.notices.len(), 1);
        registry
            .add(
                "other_tool".into(),
                "independent".into(),
                json!({}),
                Target::Mcp(0, "test".into()),
            )
            .unwrap();
        assert_eq!(registry.schemas.len(), 1);
    }

    #[test]
    fn shell_descriptions_and_path_types_match() {
        for kind in ["powershell", "cmd", "bash", "sh"] {
            let settings = Settings {
                shell_kind: kind.into(),
                ..Settings::default()
            };
            let description = bash_description(&settings).unwrap();
            assert!(description.contains(&format!("Shell type: {kind}.")));
            if kind == "powershell" {
                assert!(description.contains("Use ONLY PowerShell syntax"));
            }
            if kind == "cmd" {
                assert!(description.contains("Use ONLY Windows cmd.exe batch syntax"));
            }
        }
        let settings = Settings {
            shell_kind: "bash".into(),
            bash_path: "powershell.exe".into(),
            ..Settings::default()
        };
        assert!(
            resolve_shell(&settings)
                .unwrap_err()
                .to_string()
                .contains("不匹配")
        );
    }

    #[cfg(windows)]
    #[test]
    fn powershell_and_cmd_preserve_unicode_and_configured_directory() {
        let root = std::env::temp_dir().join(format!("Mori Shell 中文 {}", crate::model::id()));
        std::fs::create_dir_all(&root).unwrap();
        for (kind, program, script) in [
            (
                "powershell",
                "powershell.exe",
                "Set-Content -LiteralPath './中文.txt' -Value '你好 🌿' -Encoding utf8; [Console]::Write('你好 🌿')",
            ),
            ("cmd", "cmd.exe", "echo 你好>cmd.txt\necho 命令完成"),
        ] {
            let settings = Settings {
                tools_enabled: true,
                bash_enabled: true,
                shell_kind: kind.into(),
                bash_path: program.into(),
                bash_working_directory: root.to_string_lossy().into_owned(),
                ..Settings::default()
            };
            let mut registry = ToolRegistry::load(&settings).unwrap();
            assert!(registry.notices.is_empty(), "{:?}", registry.notices);
            assert_eq!(registry.schemas[0]["function"]["name"], "shell");
            assert!(registry.is_local_shell("shell"));
            let script = if kind == "powershell" {
                format!("# {}\n{script}", "长脚本".repeat(10000))
            } else {
                script.to_string()
            };
            let call = ToolCall {
                name: "shell".into(),
                arguments: json!({"command":script}).to_string(),
                ..ToolCall::default()
            };
            let output = registry
                .execute(&call, &settings, &AtomicBool::new(false))
                .unwrap();
            assert!(
                output.contains(if kind == "cmd" {
                    "命令完成"
                } else {
                    "你好 🌿"
                }),
                "{output}"
            );
            let content = std::fs::read_to_string(root.join(if kind == "cmd" {
                "cmd.txt"
            } else {
                "中文.txt"
            }))
            .unwrap();
            assert!(content.contains("你好"), "{content}");
        }
        std::fs::remove_dir_all(root).unwrap();
        let automatic = resolve_shell(&Settings::default()).unwrap();
        assert!(["powershell", "cmd", "bash"].contains(&automatic.shell_kind.as_str()));
    }

    #[test]
    fn working_directory_expands_home_and_rejects_invalid_paths() {
        let mut settings = Settings::default();
        let home = directories::UserDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();
        assert_eq!(bash_directory(&settings).unwrap(), home);
        settings.bash_working_directory = "~/".into();
        assert_eq!(bash_directory(&settings).unwrap(), home);
        settings.bash_working_directory = "relative/path".into();
        assert!(bash_directory(&settings).is_err());
        settings.bash_working_directory = home
            .join("mori-missing-directory-fixture")
            .to_string_lossy()
            .into_owned();
        assert!(bash_directory(&settings).is_err());
        let old: Settings =
            serde_json::from_str(r#"{"send_on_enter":true,"bash_path":"custom-shell"}"#).unwrap();
        assert!(old.send_on_enter);
        assert_eq!(old.bash_path, "custom-shell");
        assert_eq!(old.bash_working_directory, "~");
    }

    #[test]
    fn bash_schema_describes_the_real_host_and_home() {
        let settings = Settings {
            tools_enabled: true,
            bash_enabled: true,
            ..Settings::default()
        };
        let description = bash_description(&settings).unwrap();
        assert!(description.contains(std::env::consts::OS));
        assert!(
            description.contains(
                &bash_directory(&settings)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            )
        );
        assert!(description.contains("NOT a hosted Linux sandbox"));
        assert!(description.contains("Each call starts a fresh shell"));
    }

    #[test]
    fn bash_preserves_explicit_directory_with_spaces_and_unicode() {
        let root = std::env::temp_dir().join(format!("Mori 桌面测试 {}", crate::model::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("marker.txt"), "fixture").unwrap();
        #[cfg(windows)]
        let bash = "C:/Program Files/Git/bin/bash.exe";
        #[cfg(not(windows))]
        let bash = "bash";
        let output = run_process_in(
            bash,
            &[
                "-c".into(),
                "test -f marker.txt && printf 'correct-directory'".into(),
            ],
            None,
            &AtomicBool::new(false),
            Some(&root),
        )
        .unwrap();
        assert!(output.contains("correct-directory"), "{output}");
        // Explicit cd remains possible within a call.
        std::fs::create_dir(root.join("child")).unwrap();
        let output = run_process_in(
            bash,
            &[
                "-c".into(),
                "cd child && test -f ../marker.txt && printf 'changed-directory'".into(),
            ],
            None,
            &AtomicBool::new(false),
            Some(&root),
        )
        .unwrap();
        assert!(output.contains("changed-directory"), "{output}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn bash_tool_starts_in_the_user_home() {
        let settings = Settings {
            tools_enabled: true,
            bash_enabled: true,
            bash_path: "C:/Program Files/Git/bin/bash.exe".into(),
            ..Settings::default()
        };
        let mut registry = ToolRegistry::load(&settings).unwrap();
        let call = ToolCall {
            name: "bash".into(),
            arguments: r#"{"command":"pwd -W"}"#.into(),
            ..ToolCall::default()
        };
        let output = registry
            .execute(&call, &settings, &AtomicBool::new(false))
            .unwrap();
        let expected = bash_directory(&settings)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            output.to_lowercase().contains(&expected.to_lowercase()),
            "{output}; expected {expected}"
        );
    }

    #[test]
    fn unicode_output_preserves_chinese_emoji_and_bom_encodings() {
        let text = "中国标准时间 · 你好 🌿\n";
        assert_eq!(decode_output(text.as_bytes()), text);
        let mut utf8 = vec![0xef, 0xbb, 0xbf];
        utf8.extend_from_slice(text.as_bytes());
        assert_eq!(decode_output(&utf8), text);
        for little_endian in [true, false] {
            let mut bytes = if little_endian {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&if little_endian {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            assert_eq!(decode_output(&bytes), text);
        }
        assert_eq!(decode_output(&[]), "");
    }

    #[cfg(windows)]
    #[test]
    fn windows_gbk_timezone_decodes_without_replacement_characters() {
        let bytes = b"Wed Oct 14 13:50:09 \xd6\xd0\xb9\xfa\xb1\xea\xd7\xbc\xca\xb1\xbc\xe4 2026\n";
        let text = "Wed Oct 14 13:50:09 中国标准时间 2026\n";
        assert_eq!(decode_windows_output(bytes, 936).as_deref(), Some(text));
        // SAFETY: GetACP has no arguments or preconditions.
        if unsafe { GetACP() } == 936 {
            assert_eq!(decode_output(bytes), text);
        }
    }

    #[test]
    fn bash_stdout_stderr_and_date_use_the_same_output_decoder() {
        #[cfg(windows)]
        let bash = "C:/Program Files/Git/bin/bash.exe";
        #[cfg(not(windows))]
        let bash = "bash";
        let output = run_process(
            bash,
            &[
                "-lc".into(),
                "printf '你好 🌿\\n'; printf '错误提示\\n' >&2; date".into(),
            ],
            None,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(output.contains("你好 🌿"));
        assert!(output.contains("错误提示"));
        assert!(!output.contains('\u{fffd}'));
        #[cfg(windows)]
        {
            // SAFETY: GetACP has no arguments or preconditions.
            if unsafe { GetACP() } == 936 {
                let output = run_process(bash, &["-lc".into(), "printf 'UTF-8: 你好\\n'; printf '\\326\\320\\271\\372\\261\\352\\327\\274\\312\\261\\274\\344\\n' >&2".into()], None, &AtomicBool::new(false)).unwrap();
                assert!(output.contains("UTF-8: 你好"));
                assert!(output.contains("中国标准时间"));
                assert!(!output.contains('\u{fffd}'));
            }
        }
    }

    #[test]
    fn truncated_utf8_keeps_complete_characters_instead_of_legacy_decoding() {
        #[cfg(windows)]
        let bash = "C:/Program Files/Git/bin/bash.exe";
        #[cfg(not(windows))]
        let bash = "bash";
        // 131072 bytes ends in the second byte of a three-byte UTF-8 character.
        let output = run_process(
            bash,
            &["-lc".into(), "printf '你%.0s' {1..45000}".into()],
            None,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(output.contains("你你你"));
        assert!(output.contains("output truncated"));
        assert!(!output.contains('\u{fffd}'));
    }
}
