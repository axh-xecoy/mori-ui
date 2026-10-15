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
            this.add("bash".into(), bash_description(settings)?, json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}), Target::Bash)?;
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
                let desktop = desktop_directory()?;
                run_bash(&settings.bash_path, command, cancel, &desktop)
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

fn desktop_directory() -> Result<std::path::PathBuf> {
    let desktop = directories::UserDirs::new()
        .and_then(|dirs| dirs.desktop_dir().map(std::path::Path::to_path_buf))
        .context("无法定位系统桌面目录")?;
    if !desktop.is_dir() {
        bail!("桌面目录不存在或不可访问：{}", desktop.display());
    }
    Ok(desktop)
}

fn bash_description(settings: &Settings) -> Result<String> {
    let desktop = desktop_directory()?.to_string_lossy().replace('\\', "/");
    Ok(format!(
        "Execute Bash on the user's local machine. OS: {} ({}). Shell executable: {}. Initial working directory on EVERY call: {} (the actual system Desktop). Each call starts a fresh shell; cd does not persist between calls. This is a local desktop application, NOT a hosted Linux sandbox. Do not assume /mnt/data, /home/oai/share, or /tmp are available. On Windows use quoted forward-slash drive paths such as '{}' or relative paths from Desktop, e.g. cat > './cute_snake_game.html' <<'EOF'. Check pwd and available tools when needed; never invent directories. Scripts are delivered as UTF-8; write text files as UTF-8, and use a quoted heredoc delimiter for literal content. {}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        settings.bash_path,
        desktop,
        desktop,
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
    let mut child = command
        .args(args)
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
    fn bash_schema_describes_the_real_host_and_desktop() {
        let settings = Settings {
            tools_enabled: true,
            bash_enabled: true,
            ..Settings::default()
        };
        let registry = ToolRegistry::load(&settings).unwrap();
        let description = registry.schemas[0]["function"]["description"]
            .as_str()
            .unwrap();
        assert!(description.contains(std::env::consts::OS));
        assert!(
            description.contains(
                &desktop_directory()
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            )
        );
        assert!(description.contains("NOT a hosted Linux sandbox"));
        assert!(description.contains("relative paths from Desktop"));
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
    fn bash_tool_starts_in_the_actual_windows_desktop() {
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
        let expected = desktop_directory()
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
