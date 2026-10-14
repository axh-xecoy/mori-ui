use super::*;

pub fn start(
    handle: AnyWindowHandle,
    view: Entity<Mori>,
    directory: std::path::PathBuf,
    cx: &mut App,
) {
    std::fs::create_dir_all(&directory).expect("snapshot directory");
    view.update(cx, |this, _| {
        this.store = None;
        this.library.conversations.clear();
        this.library.active_id = None;
    });
    cx.spawn(async move |cx| {
        let input_only=std::env::var_os("MORI_INPUT_CHECK_ONLY").is_some();
        let tools_only=std::env::var_os("MORI_TOOLS_CHECK_ONLY").is_some();
        for (stage, name) in ["welcome", "chat", "models", "assistants", "tools", "settings", "dark", "compact", "rename", "deleted", "restored", "provider-saved", "math", "math-dark", "math-compact", "settings-general", "settings-connections", "settings-data", "settings-english", "composer-images", "sidebar-wide", "settings-compact", "connections-compact", "streaming-reply", "streaming-long", "conversation-menu", "thinking-menu", "composer-narrow", "composer-narrow-models", "tool-collapsed", "tool-expanded", "tool-expanded-dark", "code-actions", "code-actions-dark", "code-actions-compact", "notice-visible", "notice-exiting", "notice-dismissed", "clipboard-attachments", "clipboard-ready", "model-revalidated", "model-empty", "sent-image", "sent-image-dark"].into_iter().enumerate().filter(|(stage,_)| (!input_only||*stage==19)&&(!tools_only||*stage>=29)) {
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
                        41 => {for p in &mut this.library.providers {p.models.clear();}this.new_chat(window,cx);assert!(this.current().unwrap().model.is_empty());}, 42 => {window.resize(size(px(1280.),px(860.)));this.new_chat(window,cx);this.toast=None;this.attachments.clear();this.composer.update(cx,|s,cx|s.set_value("",window,cx));let mut message=Message::new("user","帮我看看这张图片。");message.attachments.push(Attachment {name:"粘贴图片.png".into(),path:std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/mori-128.png").to_string_lossy().into(),mime:"image/png".into(),text:None});this.current_mut().unwrap().messages.push(message);}, 43 => {this.library.settings.dark=true;}, _ => {}
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
        cx.update(|cx|cx.quit());
    }).detach();
}
