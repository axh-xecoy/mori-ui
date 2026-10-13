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
        for (stage, name) in ["welcome", "chat", "models", "assistants", "tools", "settings", "dark", "compact", "rename", "deleted", "restored"].into_iter().enumerate() {
            let _ = handle.update(cx, |_, window, cx| {
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
                            let id=this.library.conversations[0].id.clone(); this.delete_conversation(&id,window,cx); assert!(this.library.conversations.is_empty());
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
                        }, _ => {}
                    }
                    Theme::change(if this.library.settings.dark {ThemeMode::Dark} else {ThemeMode::Light},Some(window),cx);
                    Mori::tint(cx,this.library.settings.dark); cx.notify();
                });
                let _ = window.draw(cx);
            });
            cx.background_executor().timer(Duration::from_millis(600)).await;
            let path=directory.join(format!("{name}.png"));
            let result=handle.update(cx, |_, window, cx| { let _ = window.draw(cx); window.render_to_image().and_then(|image|image.save(&path).map_err(Into::into)) });
            eprintln!("snapshot {name}: {result:?}");
        }
        cx.update(|cx|cx.quit());
    }).detach();
}
