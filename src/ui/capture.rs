use super::*;

thread_local! {
    static TOOL_SCROLLS: std::cell::RefCell<std::collections::HashMap<String, ScrollHandle>> = Default::default();
    static CODE_EDITORS: std::cell::RefCell<std::collections::HashMap<String, Entity<gpui_kit::component::input::EditorState>>> = Default::default();
    static ROW_RENDERS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};
    static EXPORT_PROBE: std::cell::Cell<Option<usize>> = const {std::cell::Cell::new(None)};
}

pub(super) fn record_export_click() -> bool {
    EXPORT_PROBE.with(|count| match count.get() {
        Some(n) => {
            count.set(Some(n + 1));
            true
        }
        None => false,
    })
}

pub(super) fn record_code_editor(
    key: &str,
    state: &Entity<gpui_kit::component::input::EditorState>,
) {
    CODE_EDITORS.with(|editors| {
        editors.borrow_mut().insert(key.into(), state.clone());
    });
}
pub(super) fn record_message_row() {
    ROW_RENDERS.with(|count| count.set(count.get() + 1));
}

pub(super) fn record_tool_scroll(key: &str, handle: &ScrollHandle) {
    TOOL_SCROLLS.with(|handles| {
        handles.borrow_mut().insert(key.into(), handle.clone());
    });
}

pub fn start(
    handle: AnyWindowHandle,
    view: Entity<Mori>,
    directory: std::path::PathBuf,
    cx: &mut App,
) {
    std::fs::create_dir_all(&directory).expect("snapshot directory");
    let failure_path = directory.join("failure.txt");
    std::panic::set_hook(Box::new(move |info| {
        let _ = std::fs::write(&failure_path, info.to_string());
        eprintln!("{info}");
    }));
    view.update(cx, |this, _| {
        this.store = None;
        // Explicit test fixture; production starts with no connections.
        let mut fixture = Provider::custom();
        fixture.id = "ollama".into();
        fixture.name = "测试连接".into();
        fixture.protocol = Protocol::Ollama;
        fixture.base_url = "http://localhost:11434".into();
        this.library.providers = vec![fixture];
        this.library.conversations.clear();
        this.library.active_id = None;
    });
    cx.spawn(async move |cx| {
        let input_only=std::env::var_os("MORI_INPUT_CHECK_ONLY").is_some();
        let tools_only=std::env::var_os("MORI_TOOLS_CHECK_ONLY").is_some();
        let performance_only=std::env::var_os("MORI_PERF_CHECK_ONLY").is_some();
        for (stage, name) in ["welcome", "chat", "models", "assistants", "tools", "settings", "dark", "compact", "rename", "deleted", "restored", "provider-saved", "math", "math-dark", "math-compact", "settings-general", "settings-connections", "settings-data", "settings-english", "composer-images", "sidebar-wide", "settings-compact", "connections-compact", "streaming-reply", "streaming-long", "conversation-menu", "thinking-menu", "composer-narrow", "composer-narrow-models", "tool-collapsed", "tool-expanded", "tool-expanded-dark", "code-actions", "code-actions-dark", "code-actions-compact", "notice-visible", "notice-exiting", "notice-dismissed", "clipboard-attachments", "clipboard-ready", "model-revalidated", "model-empty", "sent-image", "sent-image-dark", "user-newlines", "user-newlines-dark", "user-newlines-compact", "model-parameters", "model-alias-chat", "model-parameters-compact", "model-parameters-english", "models-managed", "thinking-bar", "thinking-bar-expanded", "thinking-bar-dark", "llama-backend-settings"].into_iter().enumerate().filter(|(stage,_)| (!input_only||*stage==19)&&(!tools_only||*stage>=29)&&(!performance_only||*stage==29)) {
            let _ = handle.update(cx, |_, window, cx| {
                if stage==27 {window.dispatch_event(PlatformInput::KeyDown(KeyDownEvent {keystroke:Keystroke::parse("escape").unwrap(),is_held:false,prefer_character_input:false}),cx);}
                view.update(cx, |this, cx| {
                    this.page = Page::Chat; this.model_picker = false; this.library.settings.dark = false;
                    if stage == 1 {
                        let mut conversation = Conversation::new("openai".into(), "gpt-4.1".into(), "general".into());
                        conversation.title = "留一点时间给自己".into();
                        conversation.messages.push(Message::new("user", "周末想放松一下，有什么建议？"));
                        conversation.messages.push(Message::new("assistant", "当然。可以为自己留一段不用赶时间的周末。\n\n### 三个小小的提案\n\n- **去公园散步**：带一杯喜欢的咖啡，让思绪慢下来。\n- **做一道新菜**：从挑选食材开始，享受动手的过程。\n- **读一章书**：不用制定目标，读到有意思的地方就好。\n\n你更喜欢待在家里，还是出去走走？"));
                        let id = conversation.id.clone(); this.library.conversations.push(conversation); this.library.active_id = Some(id);
                    }
                    match stage {
                        2 => this.model_picker=true, 3 => this.page=Page::Assistants, 4 => this.page=Page::Tools, 5 => this.page=Page::Settings, 6 => this.library.settings.dark=true,
                        7 => { window.resize(size(px(900.),px(640.))); this.library.active_id=None; },
                        8 => { window.resize(size(px(1280.),px(860.))); let id=this.library.conversations[0].id.clone(); this.rename_conversation(id,window,cx); assert!(this.rename); },
                        9 => {
                            this.title_input.update(cx,|s,cx|s.set_value("周末的灵感",window,cx));
                            this.commit_title(window,cx);
                            assert_eq!(this.library.conversations[0].title,"周末的灵感");
                            let id=this.library.conversations[0].id.clone(); this.delete_conversation(&id,window,cx); assert!(this.library.conversations.is_empty()); assert!(this.show_undo_delete());
                        },
                        10 => {
                            this.undo_delete(window,cx); assert_eq!(this.library.conversations.len(),1);
                            let original = this.library.active_id.clone().unwrap();
                            this.new_chat(window,cx);
                            let other = this.library.active_id.clone().unwrap();
                            let attachment = Attachment { name:"regression.txt".into(), path:"unused-test-path".into(), mime:"text/plain".into(), text:Some("Regression attachment".into()) };
                            this.work_tx.send(Work::Attach(original.clone(),Ok(vec![attachment.clone()]))).unwrap();
                            this.poll(cx);
                            assert!(this.attachments.is_empty(),"Background attachments must not leak into another conversation");
                            assert_eq!(this.library.conversations.iter().find(|c|c.id==original).unwrap().draft_attachments.len(),1);
                            this.work_tx.send(Work::Attach("deleted-conversation".into(),Ok(vec![attachment]))).unwrap();
                            this.poll(cx);
                            assert!(this.attachments.is_empty(),"Deleted targets must not redirect attachments");
                            this.library.conversations.retain(|c|c.id!=other);
                            this.select_chat(original,window,cx);
                            assert_eq!(this.attachments.len(),1,"Original conversation restores its imported attachment");
                            this.attachments.clear(); this.stash_draft(cx); this.toast=None;
                        },
                        11 => {
                            let id=this.library.active_id.clone().unwrap(); this.delete_conversation(&id,window,cx);
                            assert!(this.show_undo_delete());
                            this.load_provider(0,window,cx);
                            this.provider_url.update(cx,|s,cx|s.set_value("http://localhost:11434/v1",window,cx));
                            assert!(this.save_provider(window,cx));
                            assert!(this.last_deleted.is_some()); assert!(!this.show_undo_delete(),"Save provider notices must not show an unrelated undo action");
                            assert_eq!(this.provider_url.read(cx).value().as_ref(),"http://localhost:11434");
                            assert_eq!(this.library.providers[0].base_url,"http://localhost:11434");
                            this.page=Page::Settings;
                            this.settings_tab=SettingsTab::Connections;
                        },
                        12 => {
                            this.new_chat(window,cx); this.toast=None;
                            let content=r"### 体积与高度
我们需要找出 $V_{\text{水2}}$ 中圆锥与圆柱的体积。

圆锥剩余部分：$$V_{\text{锥剩}}=V_{\text{锥总}}-\frac{1}{8}V_{\text{锥总}}=\frac{7}{8}V_{\text{锥总}}$$
圆柱体积：$$V_{\text{柱}}=\frac{30}{8}V_{\text{锥总}}-\frac{7}{8}V_{\text{锥总}}=\frac{23}{8}V_{\text{锥总}}$$
代入 $V_{\text{锥总}}=4\pi r^2$：$$\pi r^2 h_{\text{柱}}=\frac{23}{8}\times4\pi r^2$$
所以 $h_{\text{柱}}=11.5\text{ 厘米}$。

代码中的 `$$x^2$$` 保持原样。";
                            let c=this.current_mut().unwrap(); c.title="数学公式".into(); c.messages.push(Message::new("assistant",content));
                        },
                        13 => this.library.settings.dark=true,
                        14 => window.resize(size(px(900.),px(640.))),
                        15 => {window.resize(size(px(1280.),px(860.)));this.page=Page::Settings;this.settings_tab=SettingsTab::General;},
                        16 => {this.page=Page::Settings;this.settings_tab=SettingsTab::Connections;},
                        17 => {this.page=Page::Settings;this.settings_tab=SettingsTab::Data;},
                        18 => {this.page=Page::Settings;this.settings_tab=SettingsTab::General;this.set_language("en",window,cx);this.library.settings.send_on_enter=true;this.composer.update(cx,|s,cx|s.set_submit_on_enter(true,cx));},
                        19 => {
                            this.new_chat(window,cx);this.toast=None;
                            this.composer.update(cx,|s,cx|s.set_value("Describe this image and summarize the attached notes.",window,cx));
                            let root=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                            this.attach_paths(vec![root.join("assets/mori-128.png"),root.join("README.md")],window,cx);
                        },
                        20 => {assert_eq!(this.attachments.len(),2);assert!(this.attachments.iter().any(|a|a.mime.starts_with("image/")));this.library.settings.sidebar_width=360.;},
                        21 => {window.resize(size(px(900.),px(640.)));this.page=Page::Settings;this.settings_tab=SettingsTab::General;},
                        22 => {this.page=Page::Settings;this.settings_tab=SettingsTab::Connections;this.provider_picker=false;},
                        23 => {
                            window.resize(size(px(1280.),px(860.))); this.library.settings.sidebar_width=252.;this.set_language("zh-CN",window,cx);
                            this.new_chat(window,cx);this.toast=None;this.attachments.clear();this.library.settings.thinking=Some(true);
                            let c=this.current_mut().unwrap();c.title="让回复平稳展开".into();c.model="qwen3.6:35b".into();
                            c.messages.push(Message::new("user","帮我安排一个轻松的周末。"));
                            let mut reply=Message::new("assistant","可以先给自己留一段空白的时间。\n\n### 周末计划\n\n- 周六早上：去公园散步。");reply.reasoning="考虑放松、出行和休息之间的平衡。".into();reply.elapsed_ms=3800;c.messages.push(reply);
                            let conversation=c.id.clone(); let (_,rx)=mpsc::channel();this.generation=Some(Generation {conversation,rx,cancel:Arc::new(AtomicBool::new(false))});this.status="正在回复…".into();
                        },
                        24 => {let c=this.current_mut().unwrap();let reply=c.messages.last_mut().unwrap();reply.content.push_str("\n- 周六下午：在家读一章书。\n- 周日：给喜欢的人做一顿饭。\n\n不用安排得太满，休息也是计划的一部分。");reply.elapsed_ms=8900;reply.tokens=215;},
                        25 => {this.generation=None;this.status.clear();let c=this.current().unwrap().clone();this.library.conversations=vec![c];},
                        26 => {},
                        27 => {this.generation=None;this.status.clear();this.library.settings.sidebar_width=380.;window.resize(size(px(900.),px(640.)));this.set_language("en",window,cx);this.library.settings.tools_enabled=true;},
                        28 => {this.model_picker=true;},
                        29 => {
                            window.resize(size(px(1280.),px(860.)));this.library.settings.sidebar_width=252.;this.set_language("zh-CN",window,cx);this.model_picker=false;
                            this.new_chat(window,cx);this.toast=None;
                            let mut reply=Message::new("assistant","命令已完成。详细输出可以按需展开查看。");
                            reply.tools.push(ToolCall {id:"long-bash-output".into(),name:"bash".into(),arguments:r#"{"command":"find . -type f"}"#.into(),result:Some((1..1500).map(|n|format!("第 {n} 行：文件与目录信息，中文正常显示 🌿\n")).collect::<String>())});
                            let c=this.current_mut().unwrap();c.title="长输出折叠检查".into();c.messages=vec![reply];
                            let key=format!("tool-{}-long-bash-output",c.messages[0].id);assert!(!this.expanded.contains(&key));
                        },
                        30 => {let key=format!("tool-{}-long-bash-output",this.current().unwrap().messages[0].id);this.expanded.insert(key);},
                        31 => {this.library.settings.dark=true;},
                        32 => {
                            this.new_chat(window,cx);this.toast=None;
                            let c=this.current_mut().unwrap();c.title="代码与消息操作".into();
                            c.messages.push(Message::new("user","用 Python 写一个简单的问候函数，再给出一份 JSON 配置。"));
                            c.messages.push(Message::new("assistant",r#"可以这样写，复制按钮只复制代码，下载会保存为对应类型的文件。

```python
# 简单问候
from datetime import datetime

def greet(name: str) -> str:
    return f"你好，{name} 🌿"

print(greet("Mori"))
```

```json
{"name": "Mori", "thinking": true, "temperature": 0.7}
```
"#));
                        },
                        33 => {this.library.settings.dark=true;},
                        34 => {window.resize(size(px(900.),px(640.)));},
                        35 => {window.resize(size(px(1280.),px(860.)));this.library.settings.sidebar_width=252.;this.show_toast("偏好已保存".into());},
                        36 => {assert!(this.toast.is_some());this.toast_started=Some(std::time::Instant::now()-Duration::from_secs(6));assert!(this.tick_toast());assert!(this.toast_closing.is_some());},
                        37 => {this.toast_closing=Some(std::time::Instant::now()-Duration::from_millis(400));assert!(this.tick_toast());assert!(this.toast.is_none());
                            this.show_toast("偏好已保存".into());this.dismiss_toast();let old=this.toast_revision;this.show_toast("偏好已保存".into());assert!(this.toast_closing.is_none());assert!(this.toast_revision>old);this.toast=None;
                        },
                        38 => {
                            this.new_chat(window,cx);this.toast=None;
                            this.composer.update(cx,|s,cx|s.set_value("保持已有文字",window,cx));
                            let image=Image::from_bytes(ImageFormat::Png,include_bytes!("../../assets/mori-128.png").to_vec());
                            let project=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                            let paths=ExternalPaths(vec![project.join("README.md"),project.join("assets/mori-128.png")].into());
                            let item=ClipboardItem {entries:vec![ClipboardEntry::Image(image),ClipboardEntry::ExternalPaths(paths),ClipboardEntry::String(ClipboardString::new("不要插入替代文本".into()))]};
                            this.paste_attachments(item,window,cx);assert_eq!(this.attachments_loading,1);this.send(window,cx);assert!(this.generation.is_none());assert!(this.current().unwrap().messages.is_empty());
                        },
                        39 => {assert_eq!(this.attachments_loading,0);assert_eq!(this.attachments.len(),2);assert!(this.attachments.iter().any(|a|a.name=="mori-128.png"));assert_eq!(this.composer.read(cx).value().as_ref(),"保持已有文字");},
                        40 => {
                            let p=&mut this.library.providers[0];p.enabled=true;p.models=vec!["available-model".into()];let pid=p.id.clone();
                            let c=this.current_mut().unwrap();c.provider_id=pid.clone();c.model="removed-model".into();let old=c.id.clone();
                            this.new_chat(window,cx);assert_eq!(this.current().unwrap().model,"available-model");assert_eq!(this.library.conversations.iter().find(|c|c.id==old).unwrap().model,"removed-model");
                        },
                        41 => {for p in &mut this.library.providers {p.models.clear();}this.new_chat(window,cx);assert!(this.current().unwrap().model.is_empty());}, 42 => {window.resize(size(px(1280.),px(860.)));this.new_chat(window,cx);this.toast=None;this.attachments.clear();this.composer.update(cx,|s,cx|s.set_value("",window,cx));let mut message=Message::new("user","帮我看看这张图片。");message.attachments.push(Attachment {name:"粘贴图片.png".into(),path:std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/mori-128.png").to_string_lossy().into(),mime:"image/png".into(),text:None});this.current_mut().unwrap().messages.push(message);}, 43 => {this.library.settings.dark=true;}, 44 => {this.new_chat(window,cx);this.toast=None;this.attachments.clear();let c=this.current_mut().unwrap();c.messages.push(Message::new("user","第一行：周末计划\n第二行：去公园散步\n\n空行后的第三行\n**这些符号是输入的原文**\n<div>原样显示 & 不解析为标签</div>"));c.messages.push(Message::new("assistant","### 回复仍支持 Markdown\n\n- 第一项\n- 第二项"));},45 => {this.library.settings.dark=true;},46 => {window.resize(size(px(900.),px(640.)));},47 => {window.resize(size(px(1280.),px(860.)));this.page=Page::Settings;this.settings_tab=SettingsTab::Connections;this.library.providers[0].models=vec!["qwen3:8b".into()];this.load_provider(0,window,cx);this.edit_model("qwen3:8b",window,cx);this.model_name_input.update(cx,|s,cx|s.set_value("千问3 8B",window,cx));this.model_temperature.update(cx,|s,cx|s.set_value("0.6",window,cx));this.model_top_p.update(cx,|s,cx|s.set_value("0.9",window,cx));this.model_max_tokens.update(cx,|s,cx|s.set_value("32768",window,cx));this.model_parameters.update(cx,|s,cx|s.set_value("{\n  \"top_k\": 40,\n  \"seed\": 42,\n  \"options\": { \"num_ctx\": 65536 }\n}",window,cx));},48 => {this.save_model(cx);assert_eq!(this.library.providers[0].model_name("qwen3:8b"),"千问3 8B");assert_eq!(this.library.providers[0].model_configs["qwen3:8b"].parameters["top_p"],0.9);this.new_chat(window,cx);this.toast=None;let c=this.current_mut().unwrap();c.provider_id="ollama".into();c.model="qwen3:8b".into();c.messages.push(Message::new("user","介绍一下你自己。"));let mut reply=Message::new("assistant","你好，我是千问，很高兴和你聊天。");reply.provider_id="ollama".into();reply.model_id="qwen3:8b".into();c.messages.push(reply);assert_eq!(this.reply_model_name(this.current().unwrap().messages.last().unwrap()),"千问3 8B");},49 => {this.page=Page::Settings;this.settings_tab=SettingsTab::Connections;this.edit_model("qwen3:8b",window,cx);window.resize(size(px(900.),px(640.)));},50 => {window.resize(size(px(1280.),px(860.)));this.set_language("en",window,cx);this.page=Page::Settings;this.settings_tab=SettingsTab::Connections;},51 => {this.set_language("zh-CN",window,cx);this.model_editor=false;this.page=Page::Settings;this.settings_tab=SettingsTab::Connections;},52 => {this.new_chat(window,cx);this.toast=None;let c=this.current_mut().unwrap();c.messages.push(Message::new("user","帮我整理一个清晰的周末计划。"));let mut m=Message::new("assistant","可以把周末安排得松弛一些：\n\n- 早上去公园散步\n- 下午读书，留一点自由时间\n- 晚上和家人一起吃饭");m.reasoning="先考虑休息、活动和陪伴之间的平衡，再把活动分散到不同时间段。\n\n计划应留出弹性，避免把休息日排得过满。".into();c.messages.push(m);},53 => {let id=this.current().unwrap().messages.last().unwrap().id.clone();this.expanded.insert(id);},54 => {this.library.settings.dark=true;let id=this.current().unwrap().messages.last().unwrap().id.clone();this.expanded.remove(&id);},55 => {this.library.providers[0].name="llama.cpp".into();this.library.providers[0].base_url="http://localhost:8080/v1".into();this.library.providers[0].protocol=Protocol::OpenAi;this.library.providers[0].backend=OpenAiBackend::LlamaCpp;this.page=Page::Settings;this.settings_tab=SettingsTab::Connections;this.load_provider(0,window,cx);}, _ => {}
                    }
                    Theme::change(if this.library.settings.dark {ThemeMode::Dark} else {ThemeMode::Light},Some(window),cx);
                    Mori::tint(cx,this.library.settings.dark); cx.notify();
                });
                let _ = window.draw(cx);
                if stage==25 || stage==26 {
                    if stage==26 { window.dispatch_event(PlatformInput::KeyDown(KeyDownEvent {keystroke:Keystroke::parse("escape").unwrap(),is_held:false,prefer_character_input:false}),cx); }
                    let position=if stage==25 {point(px(210.),px(400.))} else {point(px(545.),px(760.))};
                    window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent {button:MouseButton::Left,position,..Default::default()}),cx);
                    window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent {button:MouseButton::Left,position,..Default::default()}),cx);
                }
            });
            cx.background_executor().timer(Duration::from_millis(if stage==36 {100} else {600})).await;
            let path=directory.join(format!("{name}.png"));
            let result=handle.update(cx, |_, window, cx| { let _ = window.draw(cx); window.render_to_image().and_then(|image|image.save(&path).map_err(Into::into)) });
            eprintln!("snapshot {name}: {result:?}");
        }
        // Exercise the real textarea action dispatch, with no model selected so
        // submission is observable without sending a network request.
        for (enter,secondary,shift,submitted) in [(false,false,false,false),(true,false,false,true),(true,false,true,false),(false,true,false,true)] {
            let _=handle.update(cx,|_,window,cx| {
                view.update(cx,|this,cx| {
                    this.page=Page::Chat;this.rename=false;this.model_picker=false;this.image_mode=false;this.attachments.clear();
                    this.current_mut().unwrap().model.clear();this.library.settings.send_on_enter=enter;
                    this.composer.update(cx,|s,cx|{s.set_submit_on_enter(enter,cx);s.set_value("keyboard regression",window,cx);s.focus(window,cx);});
                    cx.notify();
                });
                let _=window.draw(cx);
            });
            cx.background_executor().timer(Duration::from_millis(100)).await;
            let _=handle.update(cx,|_,window,cx| {
                let _=window.draw(cx);
                let action=gpui_kit::component::input::Enter {secondary,shift};
                let focus=view.read(cx).composer.read(cx).focus_handle(cx);
                eprintln!("textarea action available={} focused={}",window.is_action_available_in(&action,&focus),focus.is_focused(window));
                focus.dispatch_action(&action,window,cx);
            });
            cx.background_executor().timer(Duration::from_millis(100)).await;
            let result=handle.update(cx,|_,_,cx|view.update(cx,|this,cx| {
                assert_eq!(this.model_picker,submitted,"Enter routing must match the configured send mode");
                assert!(this.generation.is_none());
                let value=this.composer.read(cx).value();
                assert_eq!(value.contains('\n'),!enter||shift,"Only newline actions may insert a newline");
                assert_eq!(value.replace('\n',""),"keyboard regression");
            }));
            eprintln!("keyboard regression enter={enter} ctrl={secondary} shift={shift}: {result:?}");
        }
        let _=handle.update(cx,|_,window,cx| {
            view.update(cx,|this,cx|{this.library.settings.sidebar_width=252.;window.resize(size(px(1280.),px(860.)));cx.notify();});
            let _=window.draw(cx);
        });
        cx.background_executor().timer(Duration::from_millis(100)).await;
        let _=handle.update(cx,|_,window,cx| {
            let _=window.draw(cx);
            window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent {button:MouseButton::Left,position:point(px(254.),px(200.)),..Default::default()}),cx);
            window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent {position:point(px(340.),px(200.)),pressed_button:Some(MouseButton::Left),..Default::default()}),cx);
            window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent {button:MouseButton::Left,position:point(px(340.),px(200.)),..Default::default()}),cx);
        });
        cx.background_executor().timer(Duration::from_millis(100)).await;
        let result=handle.update(cx,|_,_,cx|view.update(cx,|this,_|{assert_eq!(this.library.settings.sidebar_width,340.);assert!(!this.resizing_sidebar);}));
        eprintln!("sidebar drag regression: {result:?}");
        let (stream_tx,mut reply)=handle.update(cx,|_,window,cx|{let result=view.update(cx,|this,cx| {
            this.page=Page::Chat;this.model_picker=false;this.rename=false;
            let c=this.current_mut().unwrap();c.messages.clear();
            let reply=Message::new("assistant","A stable paragraph.\n\n".repeat(100));c.messages.push(reply.clone());
            let conversation=c.id.clone();let (tx,rx)=mpsc::channel();this.generation=Some(Generation {conversation,rx,cancel:Arc::new(AtomicBool::new(false))});
            this.scroll.scroll_to_bottom();cx.notify();(tx,reply)
        });let _=window.draw(cx);result}).unwrap();
        cx.background_executor().timer(Duration::from_millis(100)).await;
        let _=handle.update(cx,|_,_,cx|{let scroll=&view.read(cx).scroll;eprintln!("before stream offset={:?} max={:?}",scroll.offset(),scroll.max_offset());});
        reply.content.push_str("Another paragraph.\n\n");stream_tx.send(Event::Message(reply.clone())).unwrap();
        let _=handle.update(cx,|_,window,cx|{view.update(cx,|this,cx|this.poll(cx));let _=window.draw(cx);});
        cx.background_executor().timer(Duration::from_millis(100)).await;
        let offset=handle.update(cx,|_,window,cx|{view.update(cx,|this,cx| {
            eprintln!("after stream offset={:?} max={:?}",this.scroll.offset(),this.scroll.max_offset());
            assert!((this.scroll.offset().y+this.scroll.max_offset().y).abs()<px(2.),"Streaming must follow the bottom after layout");
            this.scroll.set_offset(point(px(0.),px(-80.)));cx.notify();
        });let _=window.draw(cx);view.read(cx).scroll.offset()}).unwrap();
        reply.content.push_str("A later paragraph.\n\n");stream_tx.send(Event::Message(reply)).unwrap();
        let _=handle.update(cx,|_,window,cx|{view.update(cx,|this,cx|this.poll(cx));let _=window.draw(cx);});
        cx.background_executor().timer(Duration::from_millis(100)).await;
        let result=handle.update(cx,|_,_,cx|view.update(cx,|this,_|{assert_eq!(this.scroll.offset(),offset,"Streaming must not pull readers away from older messages");this.generation=None;}));
        eprintln!("streaming scroll regression: {result:?}");
        let output_key=handle.update(cx,|_,window,cx| {
            let key=view.update(cx,|this,cx| {
                this.generation=None;this.status.clear();this.page=Page::Chat;this.toast=None;
                this.new_chat(window,cx);this.toast=None;
                let mut reply=Message::new("assistant","Bash 独立滚动检查");
                reply.context_note="上下文已调整：较早的 2 轮对话未参与本次回答。保留最近对话，完整记录仍在本地。".into();
                reply.tools.push(ToolCall {id:"scroll-regression".into(),name:"bash".into(),arguments:r#"{"command":"cat game.html"}"#.into(),result:Some("中文输出很长，需要独立滚动。\n".repeat(1000))});
                let key=format!("tool-{}-scroll-regression",reply.id);this.expanded.insert(key.clone());
                let c=this.current_mut().unwrap();c.messages=vec![reply,Message::new("assistant","页面下方内容。\n\n".repeat(80))];
                this.scroll.set_offset(point(px(0.),px(0.)));cx.notify();format!("output-scroll-{key}")
            });let _=window.draw(cx);key
        }).unwrap();
        cx.background_executor().timer(Duration::from_millis(200)).await;
        let result=handle.update(cx,|_,window,cx| {
            let _=window.draw(cx);
            let output=TOOL_SCROLLS.with(|handles|handles.borrow().get(&output_key).unwrap().clone());
            assert!(output.max_offset().y>px(100.),"Output must overflow its own panel");
            assert!(view.read(cx).scroll.max_offset().y>px(100.),"Fixture must also overflow the page");
            let page_offset=view.read(cx).scroll.offset();
            let position=output.bounds().origin+point(px(60.),px(50.));
            for delta in [-100.,-100000.,-100.,100000.,100.] {
                window.dispatch_event(PlatformInput::ScrollWheel(ScrollWheelEvent {position,delta:ScrollDelta::Pixels(point(px(0.),px(delta))),..Default::default()}),cx);
                let _=window.draw(cx);
                assert_eq!(view.read(cx).scroll.offset(),page_offset,"Panel wheel must not move the page, including at either edge");
                if delta == -100. { assert!(output.offset().y<px(0.),"Wheel must move the output itself"); }
            }
            output.set_offset(point(px(0.),px(0.)));let _=window.draw(cx);
            let thumb_position=point(output.bounds().right()-px(7.),output.bounds().top()+px(18.));
            window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent {button:MouseButton::Left,position:thumb_position,..Default::default()}),cx);
            window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent {position:thumb_position+point(px(0.),px(100.)),pressed_button:Some(MouseButton::Left),..Default::default()}),cx);
            window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent {button:MouseButton::Left,position:thumb_position+point(px(0.),px(100.)),..Default::default()}),cx);
            let _=window.draw(cx);
            assert!(output.offset().y<px(0.),"Dragging the scrollbar must move its output");
            assert_eq!(view.read(cx).scroll.offset(),page_offset,"Dragging the output scrollbar must not move the page");
            output.set_offset(point(px(0.),px(-140.)));let _=window.draw(cx);
            window.render_to_image().unwrap().save(directory.join("tool-output-scrolled.png")).unwrap();
        });
        eprintln!("tool output wheel containment regression: {result:?}");
        let result=handle.update(cx,|_,window,cx| {
            view.update(cx,|this,cx| {
                this.new_chat(window,cx);this.toast=None;this.generation=None;
                this.current_mut().unwrap().messages=(0..4000).map(|i|Message::new(if i%2==0 {"user"} else {"assistant"},format!("第 {i} 条历史消息。\n\n{}","这是一段用于检验虚拟列表的正常聊天内容。".repeat(8)))).collect();
                this.scroll.set_offset(point(px(0.),px(0.)));cx.notify();
            });
            ROW_RENDERS.with(|n|n.set(0));
            let started=std::time::Instant::now();let _=window.draw(cx);
            let rows=ROW_RENDERS.with(|n|n.get());
            eprintln!("performance history: 4000 messages, {rows} row constructions, first draw {:?}",started.elapsed());
            assert!(rows<80,"Only nearby messages may be constructed");
            std::fs::write(directory.join("performance-history.json"),serde_json::to_vec_pretty(&serde_json::json!({"messages":4000,"constructed_rows":rows,"first_draw_us":started.elapsed().as_micros() as u64})).unwrap()).unwrap();
        });
        eprintln!("history virtualization regression: {result:?}");
        cx.background_executor().timer(Duration::from_millis(250)).await;
        let _=handle.update(cx,|_,window,cx| {let _=window.draw(cx);window.render_to_image().unwrap().save(directory.join("performance-history.png")).unwrap();});
        let long_code=(0..10000).map(|i|format!("const greeting{i} = '你好，世界 🌿';\n")).collect::<String>();
        let _=handle.update(cx,|_,window,cx| {
            CODE_EDITORS.with(|editors|editors.borrow_mut().clear());
            view.update(cx,|this,cx| {
                this.new_chat(window,cx);this.toast=None;
                this.current_mut().unwrap().messages=vec![Message::new("assistant",format!("这是完整的长代码，支持选择、复制与下载。\n\n```javascript\n{long_code}```"))];
                this.scroll.set_offset(point(px(0.),px(0.)));cx.notify();
            });let _=window.draw(cx);
        });
        cx.background_executor().timer(Duration::from_millis(1200)).await;
        let _=handle.update(cx,|_,window,cx|{let _=window.draw(cx);});
        cx.background_executor().timer(Duration::from_millis(700)).await;
        let result=handle.update(cx,|_,window,cx| {
            let _=window.draw(cx);
            let editor=CODE_EDITORS.with(|editors|editors.borrow().values().next().expect("Long code uses a virtualized editor").clone());
            assert_eq!(editor.read(cx).value().as_ref(),long_code.trim_end_matches('\n'));
            let rows=editor.read(cx).visible_row_range().unwrap();
            eprintln!("performance code: 10000 lines, visible rows {rows:?}");
            assert!(rows.len()<40,"Long code must only lay out visible rows");
            assert!(!editor.read(cx).is_editable(),"Code preview must remain read-only");
            let page_offset=view.read(cx).scroll.offset();
            let bounds=editor.read(cx).cursor_layout().map(|(bounds,_)|bounds);
            eprintln!("long code caret layout {bounds:?}");
            let position=bounds.unwrap().origin+point(px(60.),px(80.));
            window.dispatch_event(PlatformInput::ScrollWheel(ScrollWheelEvent {position,delta:ScrollDelta::Pixels(point(px(0.),px(-200.))),..Default::default()}),cx);
            let _=window.draw(cx);
            assert!(editor.read(cx).scroll_offset().y<px(0.),"Long code must scroll internally");
            assert_eq!(view.read(cx).scroll.offset(),page_offset,"Code scrolling must not move the conversation");
            let started=std::time::Instant::now();
            view.update(cx,|this,cx|this.composer.update(cx,|state,cx|state.set_value("长代码显示期间输入仍可交互",window,cx)));
            let _=window.draw(cx);
            assert_eq!(view.read(cx).composer.read(cx).value().as_ref(),"长代码显示期间输入仍可交互");
            assert_eq!(view.read(cx).scroll.offset(),page_offset);
            eprintln!("performance code input update + draw {:?}",started.elapsed());
            std::fs::write(directory.join("performance-code.json"),serde_json::to_vec_pretty(&serde_json::json!({"code_lines":10000,"visible_rows":rows.len(),"input_update_and_draw_us":started.elapsed().as_micros() as u64})).unwrap()).unwrap();
            window.render_to_image().unwrap().save(directory.join("performance-code.png")).unwrap();
        });
        eprintln!("long code virtualization regression: {result:?}");
        for (dark,width,name) in [(false,1280.,"emphasis-header"),(false,900.,"emphasis-header-compact"),(true,1280.,"emphasis-header-dark")] {
            let _=handle.update(cx,|_,window,cx| {
                view.update(cx,|this,cx| {
                    this.new_chat(window,cx);this.toast=None;this.sidebar=false;this.library.settings.dark=dark;
                    Theme::change(if dark {ThemeMode::Dark} else {ThemeMode::Light},Some(window),cx);Mori::tint(cx,dark);
                    window.resize(size(px(width),px(860.)));
                    let c=this.current_mut().unwrap();c.title="自己想当创作者，想发自己的作品吸引粉丝\n但又总是担心产品不够好，所以希望获得更清晰的建议与行动方案。".repeat(4);
                    c.messages=vec![Message::new("assistant",r#"## 强调格式检查

正常格式：*斜体*、**粗体**、***粗斜体***。

英文格式：*Italic ABC*、**Bold ABC**、***Both ABC***。

模型多写空格：* 中文斜体 *、** 中文粗体 **、*** 中文粗斜体 ***。

中文标点：这是**“产品经理”**的定位，也是**「用户价值」**的表达。

不要把自己定位为“写代码的人”，要定位为** “为特定玩家群体提供解决方案的产品经理” **。

嵌套格式：**外层粗体与*内层斜体*共同显示**。

原样内容：`** 中文 **`、\*\*转义星号\*\*、2 * 3 * 4。

公式保持原样：$a*b*c$。
"#)];
                    this.scroll.set_offset(point(px(0.),px(0.)));cx.notify();
                });let _=window.draw(cx);
            });
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let result=handle.update(cx,|_,window,cx|{let _=window.draw(cx);window.render_to_image().unwrap().save(directory.join(format!("{name}.png"))).unwrap();});
            eprintln!("emphasis and multiline header {name}: {result:?}");
        }
        let result=handle.update(cx,|_,window,cx| {
            view.update(cx,|this,cx| {
                this.load_assistant(1,window,cx);
                let expected=this.library.assistants[1].id.clone();
                this.new_chat(window,cx);
                let first=this.current().unwrap().id.clone();
                assert_eq!(this.current().unwrap().assistant_id,expected);
                this.load_assistant(2,window,cx);this.new_chat(window,cx);
                assert_eq!(this.current().unwrap().assistant_id,this.library.assistants[2].id);
                this.select_chat(first,window,cx);
                assert_eq!(this.selected_assistant,1);
                assert_eq!(this.assistant_prompt.read(cx).value().as_ref(),this.library.assistants[1].prompt);
                this.new_chat(window,cx);
                assert_eq!(this.current().unwrap().assistant_id,expected);
                this.composer.update(cx,|s,cx|{s.set_value("",window,cx);s.focus(window,cx);});
                let conversation=this.current().unwrap().id.clone();let (_,rx)=mpsc::channel();
                this.generation=Some(Generation {conversation,rx,cancel:Arc::new(AtomicBool::new(false))});
                cx.notify();
            });let _=window.draw(cx);
            view.update(cx,|this,cx| {
                this.composer.update(cx,|state,cx|state.replace_text_in_range(None,"回复期间准备下一条消息",window,cx));
                assert_eq!(this.composer.read(cx).value().as_ref(),"回复期间准备下一条消息");
                this.send(window,cx);
                assert!(this.generation.is_some());
                assert!(this.current().unwrap().messages.is_empty());
                this.stop(cx);
                assert_eq!(this.composer.read(cx).value().as_ref(),"回复期间准备下一条消息");
                this.stash_draft(cx);
                assert_eq!(this.current().unwrap().draft,"回复期间准备下一条消息");
            });
        });
        eprintln!("assistant selection and streaming draft regression: {result:?}");
        let result=handle.update(cx,|_,window,cx| {
            view.update(cx,|this,cx| {
                this.attachments.clear();
                let target=this.current().unwrap().id.clone();
                let attachment=Attachment {name:"picture.png".into(),path:std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/mori-128.png").to_string_lossy().into(),mime:"image/png".into(),text:None};
                for _ in 0..2 {this.work_tx.send(Work::Attach(target.clone(),Ok(vec![attachment.clone()]))).unwrap();}
                this.poll(cx);assert_eq!(this.attachments.len(),1,"Repeated completion must not duplicate previews");
                this.new_chat(window,cx);
                for _ in 0..2 {this.work_tx.send(Work::Attach(target.clone(),Ok(vec![attachment.clone()]))).unwrap();}
                this.poll(cx);
                assert!(this.attachments.is_empty());
                assert_eq!(this.library.conversations.iter().find(|c|c.id==target).unwrap().draft_attachments.len(),1);
                this.toast=None;cx.notify();
            });
        });
        eprintln!("attachment completion deduplication regression: {result:?}");
        EXPORT_PROBE.with(|count|count.set(Some(0)));
        let _=handle.update(cx,|_,window,cx| {
            view.update(cx,|this,cx|{this.show_toast("关闭提示不应触发下方的导出按钮".into());cx.notify();});
            let _=window.draw(cx);
        });
        cx.background_executor().timer(Duration::from_millis(250)).await;
        let result=handle.update(cx,|_,window,cx| {
            let _=window.draw(cx);
            let position=point(px(1280.-39.),px(30.));
            window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent {button:MouseButton::Left,position,..Default::default()}),cx);
            window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent {button:MouseButton::Left,position,..Default::default()}),cx);
            assert!(view.read(cx).toast_closing.is_some(),"The close button must respond");
            assert_eq!(EXPORT_PROBE.with(|count|count.get()),Some(0),"Closing must not activate export underneath");
            let _=window.draw(cx);
        });
        eprintln!("toast close click containment: {result:?}");
        cx.background_executor().timer(Duration::from_millis(120)).await;
        let result=handle.update(cx,|_,window,cx| {
            let _=window.draw(cx);let position=point(px(1280.-39.),px(30.));
            window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent {button:MouseButton::Left,position,..Default::default()}),cx);
            window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent {button:MouseButton::Left,position,..Default::default()}),cx);
            assert_eq!(EXPORT_PROBE.with(|count|count.get()),Some(0),"Exit animation must keep its original click shield");
        });
        eprintln!("toast exit animation click containment: {result:?}");
        cx.background_executor().timer(Duration::from_millis(350)).await;
        let result=handle.update(cx,|_,window,cx| {
            view.update(cx,|this,cx|this.poll(cx));let _=window.draw(cx);
            assert!(view.read(cx).toast.is_none());
            let position=point(px(1280.-39.),px(30.));
            window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent {button:MouseButton::Left,position,..Default::default()}),cx);
            window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent {button:MouseButton::Left,position,..Default::default()}),cx);
            assert_eq!(EXPORT_PROBE.with(|count|count.get()),Some(1),"Export must remain clickable once the toast is gone");
        });
        eprintln!("toast shield removal restores export: {result:?}");
        EXPORT_PROBE.with(|count|count.set(None));
        let result=handle.update(cx,|_,window,cx| {
            view.update(cx,|this,cx| {
                this.library.providers.clear();this.load_provider(0,window,cx);
                assert!(this.selected_chat_provider().is_none());
                this.page=Page::Settings;this.settings_tab=SettingsTab::Connections;
                this.library.settings.dark=false;Theme::change(ThemeMode::Light,Some(window),cx);Mori::tint(cx,false);
                cx.notify();
            });let _=window.draw(cx);
            window.render_to_image().unwrap().save(directory.join("connections-empty.png")).unwrap();
            view.update(cx,|this,cx| {
                this.add_connection(window,cx);
                let p=&this.library.providers[0];assert!(p.models.is_empty());assert!(p.base_url.is_empty());assert_eq!(p.protocol,Protocol::OpenAi);
                assert!(!this.save_provider(window,cx),"A blank endpoint cannot be saved as a ready connection");
                this.toast=None;
                this.provider_name.update(cx,|s,cx|s.set_value("我的自定义连接",window,cx));
                this.provider_url.update(cx,|s,cx|s.set_value("http://localhost:11434/v1",window,cx));
                this.library.providers[0].protocol=Protocol::Ollama;
                assert!(this.save_provider(window,cx));assert_eq!(this.library.providers[0].base_url,"http://localhost:11434");
                assert!(this.library.providers[0].models.is_empty());
                this.toast=None;cx.notify();
            });let _=window.draw(cx);
            window.render_to_image().unwrap().save(directory.join("connections-custom.png")).unwrap();
        });
        eprintln!("empty connections and custom-only configuration regression: {result:?}");
        let result=handle.update(cx,|_,window,cx| {
            view.update(cx,|this,cx| {
                let first=this.library.providers[0].id.clone();this.new_chat(window,cx);
                this.current_mut().unwrap().messages.push(Message::new("user","删除连接后仍然保留"));
                let chat=this.current().unwrap().id.clone();
                this.add_connection(window,cx);let second=this.library.providers[1].id.clone();
                this.add_connection(window,cx);let third=this.library.providers[2].id.clone();
                this.remove_connection(&first,window,cx);assert_eq!(this.selected_provider,1);assert_eq!(this.library.providers[1].id,third);
                this.work_tx.send(Work::Models(first.clone(),Ok(vec!["late-model".into()]))).unwrap();
                this.poll(cx);assert_eq!(this.toast.as_deref(),Some("连接已删除，已有聊天记录保留"));
                this.remove_connection(&third,window,cx);assert_eq!(this.selected_provider,0);assert_eq!(this.library.providers[0].id,second);
                this.new_chat(window,cx);let conversation=this.current().unwrap().id.clone();let (_,rx)=mpsc::channel();
                this.generation=Some(Generation {conversation,rx,cancel:Arc::new(AtomicBool::new(false))});
                this.remove_connection(&second,window,cx);assert_eq!(this.library.providers.len(),1,"An in-use connection must not be removed");
                this.stop(cx);this.remove_connection(&second,window,cx);assert!(this.library.providers.is_empty());
                assert!(this.provider_name.read(cx).value().is_empty());assert!(this.provider_url.read(cx).value().is_empty());
                let saved=this.library.conversations.iter().find(|c|c.id==chat).unwrap();assert_eq!(saved.provider_id,first);assert_eq!(saved.messages[0].content,"删除连接后仍然保留");
                this.page=Page::Settings;this.settings_tab=SettingsTab::Connections;this.toast=None;cx.notify();
            });let _=window.draw(cx);
            window.render_to_image().unwrap().save(directory.join("connection-deleted-empty.png")).unwrap();
        });
        eprintln!("connection deletion, selection, pending discovery and history regression: {result:?}");
        std::fs::write(directory.join("passed.txt"),"All native regressions passed.\n").unwrap();
        cx.update(|cx|cx.quit());
    }).detach();
}
