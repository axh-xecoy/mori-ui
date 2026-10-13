use super::*;

impl Mori {
    fn render_sidebar(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let query = self.search.read(cx).value().to_lowercase();
        let mut conversations: Vec<_> = self
            .library
            .conversations
            .iter()
            .filter(|c| {
                query.is_empty()
                    || c.title.to_lowercase().contains(&query)
                    || c.messages
                        .iter()
                        .any(|m| m.content.to_lowercase().contains(&query))
            })
            .collect();
        conversations.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.updated.cmp(&a.updated)));
        div()
            .flex()
            .flex_col()
            .w(px(252.))
            .h_full()
            .flex_shrink_0()
            .bg(rgb(p.side))
            .border_r_1()
            .border_color(rgb(p.line))
            .p_4()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .py_3()
                    .child(img(mori_logo()).size(px(38.)).flex_shrink_0())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_xl().font_weight(FontWeight::BOLD).child("Mori"))
                            .child(label("留一点空间给灵感", p.muted)),
                    ),
            )
            .child(
                Button::new("new-chat")
                    .primary()
                    .icon(icon("plus"))
                    .label("新的对话")
                    .w_full()
                    .on_click(cx.listener(|this, _, w, cx| this.new_chat(w, cx))),
            )
            .child(Input::new(&self.search).small().prefix(icon("search")))
            .child(
                div().flex().flex_col().gap_1().children(
                    [
                        (Page::Chat, "对话", "messages-square"),
                        (Page::Assistants, "助手", "sparkles"),
                        (Page::Tools, "工具与扩展", "blocks"),
                    ]
                    .into_iter()
                    .map(|(page, title, name)| {
                        navigation_button(format!("nav-{title}"), title, name)
                            .w_full()
                            .justify_start()
                            .when(self.page == page, |b| {
                                b.bg(rgb(p.soft)).text_color(rgb(p.accent))
                            })
                            .on_click(cx.listener(move |this, _, w, cx| this.navigate(page, w, cx)))
                    }),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .child(label("你的对话", p.muted))
                    .child(label(format!("{}", conversations.len()), p.muted)),
            )
            .child(
                div()
                    .id("conversation-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(conversations.into_iter().map(|c| {
                        let selected_id = c.id.clone();
                        let rename_id = c.id.clone();
                        let delete_id = c.id.clone();
                        let active = self.library.active_id.as_ref() == Some(&c.id);
                        div()
                            .id(format!("chat-{}", c.id))
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .when(active, |d| d.bg(rgb(p.soft)))
                            .hover(|d| d.bg(rgb(p.soft)))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.select_chat(selected_id.clone(), w, cx)
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .when(c.pinned, |d| {
                                                        d.child(icon("pin").size(px(12.)))
                                                    })
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .truncate()
                                                            .child(c.title.clone()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(p.muted))
                                                    .truncate()
                                                    .child(format!(
                                                        "{} · {}",
                                                        c.updated.format("%m/%d"),
                                                        if c.model.is_empty() {
                                                            "选择一个模型"
                                                        } else {
                                                            &c.model
                                                        }
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap_1()
                                            .flex_shrink_0()
                                            .child(
                                                tiny(
                                                    format!("rename-{}", c.id),
                                                    "pencil",
                                                    "重命名会话",
                                                )
                                                .size(px(24.))
                                                .text_color(rgb(p.muted))
                                                .on_click(cx.listener(move |this, _, w, cx| {
                                                    cx.stop_propagation();
                                                    this.rename_conversation(
                                                        rename_id.clone(),
                                                        w,
                                                        cx,
                                                    );
                                                })),
                                            )
                                            .child(
                                                tiny(
                                                    format!("delete-{}", c.id),
                                                    "trash",
                                                    "删除会话",
                                                )
                                                .size(px(24.))
                                                .text_color(rgb(p.muted))
                                                .disabled(
                                                    self.generation
                                                        .as_ref()
                                                        .is_some_and(|g| g.conversation == c.id)
                                                        || self.media_busy,
                                                )
                                                .on_click(cx.listener(move |this, _, w, cx| {
                                                    cx.stop_propagation();
                                                    this.delete_conversation(&delete_id, w, cx);
                                                })),
                                            ),
                                    ),
                            )
                    })),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(rgb(p.line))
                    .pt_3()
                    .child(
                        navigation_button("settings", "设置", "settings-2")
                            .w_full()
                            .justify_start()
                            .on_click(
                                cx.listener(|this, _, w, cx| this.navigate(Page::Settings, w, cx)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .pt_3()
                            .child(div().size(px(6.)).rounded_full().bg(rgb(p.accent)))
                            .child(label("本地保存 · 你的私人空间", p.muted)),
                    ),
            )
    }
    fn render_welcome(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let compact = self.compact;
        let suggestions = [
            (
                "sparkles",
                "捕捉灵感",
                "一起想想，一件值得尝试的新事",
                "帮我想几个适合周末的有趣活动，兼顾放松和新鲜感。",
            ),
            (
                "book-open",
                "探索新知",
                "把复杂的概念，讲得简单一些",
                "用一个生活中的例子，解释什么是第一性原理。",
            ),
            (
                "pen-line",
                "打磨文字",
                "让表达更清楚，也更像你",
                "我想写一封真诚自然的感谢信，请和我一起构思。",
            ),
            (
                "languages",
                "跨越语言",
                "在不同语言里，找到合适的表达",
                "请帮我把接下来发送的文字翻译成自然流畅的英文。",
            ),
        ];
        div()
            .id("welcome-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .px_8()
            .py_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .items_center()
                    .w_full()
                    .max_w(px(680.))
                    .mx_auto()
                    .my_auto()
                    .gap(px(if compact { 16. } else { 24. }))
                    .child(
                        div()
                            .size(px(if compact { 48. } else { 72. }))
                            .rounded_full()
                            .bg(rgb(p.soft))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                icon("sprout")
                                    .size(px(if compact { 26. } else { 36. }))
                                    .text_color(rgb(p.accent)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_3()
                            .when(!compact, |d| {
                                d.child(label("A LITTLE SPACE FOR BIG IDEAS", p.accent))
                            })
                            .child(
                                div()
                                    .text_size(px(if compact { 24. } else { 30. }))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("今天，让想法慢慢生长。"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(p.muted))
                                    .child("一个问题、一张图片，或一个还没成形的灵感。"),
                            ),
                    )
                    .child(div().grid().grid_cols(2).w_full().gap_3().children(
                        suggestions.into_iter().map(|(name, title, desc, prompt)| {
                            div()
                                .id(format!("suggest-{title}"))
                                .w_full()
                                .min_w_0()
                                .p(px(if compact { 12. } else { 16. }))
                                .rounded_xl()
                                .border_1()
                                .border_color(rgb(p.line))
                                .bg(rgb(p.card))
                                .cursor_pointer()
                                .hover(|d| d.border_color(rgb(p.accent)).bg(rgb(p.soft)))
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    this.composer.update(cx, |s, cx| s.set_value(prompt, w, cx));
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(icon(name).text_color(rgb(p.accent)))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(title),
                                        ),
                                )
                                .child(div().mt_2().text_xs().text_color(rgb(p.muted)).child(desc))
                        }),
                    ))
                    .when(!compact, |d| {
                        d.child(label("支持本地模型，也连接你喜欢的 AI 服务", p.muted))
                    }),
            )
    }
    fn render_message(&self, m: &Message, index: usize, p: Palette, cx: &mut Context<Self>) -> Div {
        let user = m.role == "user";
        let text = m.content.clone();
        let speak_text = m.content.clone();
        let expanded = self.expanded.contains(&m.id);
        let reasoning_id = m.id.clone();
        div()
            .flex()
            .gap_4()
            .w_full()
            .py_5()
            .child(
                div()
                    .flex_shrink_0()
                    .size(px(32.))
                    .rounded_lg()
                    .bg(rgb(if user { p.line } else { p.soft }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        icon(if user { "user-round" } else { "sprout" }).text_color(rgb(p.accent)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(if user { "你" } else { "Mori" }),
                            )
                            .when(!user && m.elapsed_ms > 0, |d| {
                                d.child(label(
                                    format!(
                                        "{:.1}s · {} tokens",
                                        m.elapsed_ms as f64 / 1000.,
                                        m.tokens
                                    ),
                                    p.muted,
                                ))
                            }),
                    )
                    .when(!m.reasoning.is_empty(), |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    button(
                                        format!("reason-{}", m.id),
                                        if expanded {
                                            "收起思考过程"
                                        } else {
                                            "查看思考过程"
                                        },
                                        "brain",
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            if !this.expanded.remove(&reasoning_id) {
                                                this.expanded.insert(reasoning_id.clone());
                                            }
                                            cx.notify();
                                        },
                                    )),
                                )
                                .when(expanded, |d| {
                                    d.child(
                                        div()
                                            .pl_4()
                                            .border_l_2()
                                            .border_color(rgb(p.line))
                                            .text_sm()
                                            .text_color(rgb(p.muted))
                                            .child(
                                                crate::math::markdown(
                                                    format!("thinking-{}", m.id),
                                                    m.reasoning.clone(),
                                                    p.muted,
                                                )
                                                .selectable(true),
                                            ),
                                    )
                                }),
                        )
                    })
                    .when(!m.content.is_empty(), |d| {
                        d.child(
                            crate::math::markdown(format!("md-{}", m.id), &m.content, p.ink)
                                .selectable(true)
                                .w_full(),
                        )
                    })
                    .children(m.attachments.iter().map(|a| {
                        let path = a.path.clone();
                        let is_image = a.mime.starts_with("image/");
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .when(is_image, |d| {
                                d.child(
                                    img(std::path::PathBuf::from(&a.path))
                                        .max_w(px(540.))
                                        .h(px(280.))
                                        .object_fit(ObjectFit::Contain)
                                        .rounded_lg(),
                                )
                            })
                            .child(
                                button(
                                    format!("attachment-{}-{}", m.id, a.name),
                                    a.name.clone(),
                                    if is_image { "image" } else { "file-text" },
                                )
                                .on_click(move |_, _, cx| {
                                    cx.open_url(&format!("file:///{}", path.replace('\\', "/")))
                                }),
                            )
                    }))
                    .children(m.tools.iter().map(|t| {
                        card(p)
                            .gap_2()
                            .p_3()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(icon("blocks").text_color(rgb(p.accent)))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(t.name.clone()),
                                    )
                                    .child(label(
                                        if t.result.is_some() {
                                            "已处理"
                                        } else {
                                            "待确认"
                                        },
                                        p.muted,
                                    )),
                            )
                            .child(
                                TextView::markdown(
                                    format!("call-{}", t.id),
                                    format!("```json\n{}\n```", t.arguments),
                                )
                                .selectable(true),
                            )
                            .when_some(t.result.clone(), |d, r| {
                                d.child(
                                    TextView::markdown(
                                        format!("result-{}", t.id),
                                        format!(
                                            "```text\n{}\n```",
                                            r.chars().take(12000).collect::<String>()
                                        ),
                                    )
                                    .selectable(true),
                                )
                            })
                    }))
                    .when_some(m.error.clone(), |d, e| {
                        d.child(
                            div()
                                .p_3()
                                .rounded_lg()
                                .bg(rgb(if self.library.settings.dark {
                                    0x402b28
                                } else {
                                    0xfff0eb
                                }))
                                .text_sm()
                                .text_color(rgb(if self.library.settings.dark {
                                    0xffb4a5
                                } else {
                                    0xa54532
                                }))
                                .child(e),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(tiny(format!("copy-{}", m.id), "copy", "复制消息").on_click(
                                move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                                },
                            ))
                            .child(
                                tiny(
                                    format!("branch-{}", m.id),
                                    if user { "pencil" } else { "git-branch" },
                                    if user {
                                        "编辑并创建分支"
                                    } else {
                                        "从这里创建分支"
                                    },
                                )
                                .on_click(cx.listener(
                                    move |this, _, w, cx| this.branch(index, user, w, cx),
                                )),
                            )
                            .when(!user, |d| {
                                d.child(
                                    tiny(
                                        format!("speak-{}", m.id),
                                        "volume-2",
                                        "朗读（服务商需支持语音接口）",
                                    )
                                    .disabled(self.media_busy)
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| this.speak(speak_text.clone(), cx),
                                    )),
                                )
                            }),
                    ),
            )
    }
    fn render_composer(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let busy = self.generation.is_some() || self.media_busy;
        let tools = self.library.settings.tools_enabled;
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .px_8()
            .pb_4()
            .when(self.approval.is_some(), |d| {
                let a = self.approval.as_ref().unwrap();
                d.child(
                    card(p)
                        .p_4()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(icon("shield-check"))
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_sm()
                                        .child(format!("{} 请求执行", a.call.name)),
                                ),
                        )
                        .child(label(
                            "检查参数后决定；工具可能读取文件、联网或修改本机内容。",
                            p.muted,
                        ))
                        .child(
                            TextView::markdown(
                                "approval-json",
                                format!("```json\n{}\n```", a.call.arguments),
                            )
                            .selectable(true),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(button("reject", "拒绝", "x").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        if let Some(a) = this.approval.take() {
                                            let _ = a.tx.send(false);
                                        }
                                        cx.notify();
                                    },
                                )))
                                .child(
                                    Button::new("approve")
                                        .primary()
                                        .small()
                                        .label("允许这一次")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(a) = this.approval.take() {
                                                let _ = a.tx.send(true);
                                            }
                                            this.status = "正在执行工具…".into();
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .max_w(px(896.))
                    .mx_auto()
                    .p_3()
                    .rounded_xl()
                    .bg(rgb(p.card))
                    .border_1()
                    .border_color(rgb(p.line))
                    .when(!self.attachments.is_empty(), |d| {
                        d.child(div().flex().flex_wrap().gap_2().mb_2().children(
                            self.attachments.iter().enumerate().map(|(i, a)| {
                                button(format!("pending-{i}"), a.name.clone(), "x")
                                    .bg(rgb(p.soft))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.attachments.remove(i);
                                        cx.notify();
                                    }))
                            }),
                        ))
                    })
                    .child(
                        Textarea::new(&self.composer)
                            .appearance(false)
                            .bordered(false)
                            .disabled(busy)
                            .w_full(),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .mt_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        tiny("attach", "paperclip", "添加图片或文档")
                                            .disabled(busy)
                                            .on_click(
                                                cx.listener(|this, _, w, cx| this.attach(w, cx)),
                                            ),
                                    )
                                    .child(
                                        tiny(
                                            "microphone",
                                            if self.recording.is_some() {
                                                "square"
                                            } else {
                                                "mic"
                                            },
                                            "语音输入 / 结束录音",
                                        )
                                        .disabled(busy && self.recording.is_none())
                                        .when(self.recording.is_some(), |b| {
                                            b.bg(rgb(p.soft)).text_color(rgb(p.accent))
                                        })
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.microphone(cx)),
                                        ),
                                    )
                                    .child(
                                        tiny("transcribe", "audio-lines", "导入音频并转写")
                                            .disabled(busy)
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.transcribe(cx)),
                                            ),
                                    )
                                    .child(
                                        button(
                                            "image-mode",
                                            if self.image_mode {
                                                "图片生成"
                                            } else {
                                                "对话"
                                            },
                                            if self.image_mode {
                                                "image"
                                            } else {
                                                "message-circle"
                                            },
                                        )
                                        .disabled(busy)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.image_mode = !this.image_mode;
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                    .child(
                                        button(
                                            "toggle-tools",
                                            if tools { "工具已开启" } else { "工具" },
                                            "blocks",
                                        )
                                        .disabled(busy)
                                        .when(tools, |b| b.text_color(rgb(p.accent)))
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.library.settings.tools_enabled =
                                                    !this.library.settings.tools_enabled;
                                                this.persist();
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(label("Ctrl + Enter 发送", p.muted))
                                    .child(if self.generation.is_some() {
                                        Button::new("send")
                                            .primary()
                                            .icon(icon("square"))
                                            .small()
                                            .label("停止")
                                            .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
                                    } else {
                                        Button::new("send")
                                            .primary()
                                            .icon(icon("arrow-up"))
                                            .small()
                                            .label("发送")
                                            .disabled(self.media_busy)
                                            .on_click(
                                                cx.listener(|this, _, w, cx| this.send(w, cx)),
                                            )
                                    }),
                            ),
                    ),
            )
            .when(!self.status.is_empty(), |d| {
                d.child(
                    div()
                        .text_center()
                        .child(label(self.status.clone(), p.muted)),
                )
            })
            .when(self.transcript.is_some(), |d| {
                d.child(
                    button("insert-transcript", "插入音频转写", "text-cursor-input").on_click(
                        cx.listener(|this, _, w, cx| {
                            if let Some(text) = this.transcript.take() {
                                let old = this.composer.read(cx).value().to_string();
                                this.composer
                                    .update(cx, |s, cx| s.set_value(format!("{old}{text}"), w, cx));
                            }
                            cx.notify();
                        }),
                    ),
                )
            })
            .child(
                div()
                    .text_center()
                    .child(label("回答可能有误，重要信息请核实。", p.muted)),
            )
    }
    fn render_chat(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let c = self.current();
        let title = c.map(|c| c.title.as_str()).unwrap_or("新的对话");
        let model = c
            .map(|c| c.model.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("选择模型");
        let assistant = c
            .and_then(|c| {
                self.library
                    .assistants
                    .iter()
                    .find(|a| a.id == c.assistant_id)
            })
            .map(|a| a.name.as_str())
            .unwrap_or("日常助手");
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(75.))
                    .flex_shrink_0()
                    .px_6()
                    .border_b_1()
                    .border_color(rgb(p.line))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                tiny("sidebar-toggle", "panel-left", "显示或隐藏侧栏").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.sidebar = !this.sidebar;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_sm()
                                            .child(title.to_string()),
                                    )
                                    .child(label(assistant.to_string(), p.muted)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                button("model-picker", model.to_string(), "chevrons-up-down")
                                    .bg(rgb(p.soft))
                                    .text_color(rgb(p.accent))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.model_picker = !this.model_picker;
                                        cx.notify();
                                    })),
                            )
                            .when(c.is_some(), |d| {
                                d.child(tiny("pin-chat", "pin", "置顶 / 取消置顶").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        if let Some(c) = this.current_mut() {
                                            c.pinned = !c.pinned;
                                        }
                                        this.persist();
                                        cx.notify();
                                    }),
                                ))
                                .child(tiny("rename-chat", "pencil", "重命名").on_click(
                                    cx.listener(|this, _, w, cx| {
                                        let title = this
                                            .current()
                                            .map(|c| c.title.clone())
                                            .unwrap_or_default();
                                        this.title_input
                                            .update(cx, |s, cx| s.set_value(title, w, cx));
                                        this.rename = !this.rename;
                                        cx.notify();
                                    }),
                                ))
                                .child(
                                    tiny("export-chat", "download", "导出为 Markdown")
                                        .on_click(cx.listener(|this, _, _, _| this.export(false))),
                                )
                            }),
                    ),
            )
            .when(self.rename, |d| {
                d.child(
                    div()
                        .w_full()
                        .max_w(px(960.))
                        .mx_auto()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_8()
                        .py_3()
                        .child(Input::new(&self.title_input))
                        .child(
                            button("save-title", "保存名称", "check").on_click(cx.listener(
                                |this, _, w, cx| {
                                    this.commit_title(w, cx);
                                },
                            )),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(if c.is_none_or(|c| c.messages.is_empty()) {
                                self.render_welcome(p, cx).into_any_element()
                            } else {
                                div()
                                    .id("messages")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.scroll)
                                    .px_8()
                                    .child(
                                        div()
                                            .max_w(px(896.))
                                            .mx_auto()
                                            .py_4()
                                            .children(
                                                c.unwrap()
                                                    .messages
                                                    .iter()
                                                    .enumerate()
                                                    .map(|(i, m)| self.render_message(m, i, p, cx)),
                                            )
                                            .when(
                                                self.generation.is_some()
                                                    && self.approval.is_none(),
                                                |d| {
                                                    d.child(
                                                        div()
                                                            .py_4()
                                                            .text_sm()
                                                            .text_color(rgb(p.accent))
                                                            .child("●  Mori 正在思考…"),
                                                    )
                                                },
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .justify_end()
                                                    .gap_2()
                                                    .pb_3()
                                                    .child(
                                                        button("retry", "重新生成", "refresh-cw")
                                                            .disabled(
                                                                self.generation.is_some()
                                                                    || self.media_busy,
                                                            )
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| this.retry(cx),
                                                            )),
                                                    )
                                                    .child(
                                                        tiny(
                                                            "to-bottom",
                                                            "arrow-down",
                                                            "跳转到最新消息",
                                                        )
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.scroll.scroll_to_bottom();
                                                            cx.notify();
                                                        })),
                                                    ),
                                            ),
                                    )
                                    .into_any_element()
                            })
                            .child(self.render_composer(p, cx)),
                    )
                    .when(self.model_picker, |d| {
                        d.child(self.render_model_picker(p, cx))
                    }),
            )
    }
    fn render_model_picker(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("model-list")
            .w(px(290.))
            .h_full()
            .flex_shrink_0()
            .overflow_y_scroll()
            .p_4()
            .border_l_1()
            .border_color(rgb(p.line))
            .bg(rgb(p.card))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("选择模型"))
                    .child(tiny("close-picker", "x", "关闭").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.model_picker = false;
                            cx.notify();
                        },
                    ))),
            )
            .children(
                self.library
                    .providers
                    .iter()
                    .filter(|p| p.enabled)
                    .map(|provider| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(label(provider.name.clone(), p.muted))
                            .when(provider.models.is_empty(), |d| {
                                d.child(label("在设置中获取列表或输入模型 ID", p.muted))
                            })
                            .children(provider.models.iter().map(|model| {
                                let pid = provider.id.clone();
                                let model = model.clone();
                                let shown = model.clone();
                                navigation_button(format!("model-{pid}-{model}"), shown, "cpu")
                                    .w_full()
                                    .justify_start()
                                    .disabled(self.generation.is_some() || self.media_busy)
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        if this.current().is_none() {
                                            this.new_chat(w, cx);
                                        }
                                        if let Some(c) = this.current_mut() {
                                            c.provider_id = pid.clone();
                                            c.model = model.clone();
                                        }
                                        this.model_picker = false;
                                        this.persist();
                                        cx.notify();
                                    }))
                            }))
                    }),
            )
            .child(
                button("configure-models", "管理服务商", "settings-2")
                    .on_click(cx.listener(|this, _, w, cx| this.navigate(Page::Settings, w, cx))),
            )
    }
    fn render_assistants(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("assistants-page")
            .size_full()
            .max_w(px(960.))
            .mx_auto()
            .overflow_y_scroll()
            .p_8()
            .flex()
            .flex_col()
            .gap_6()
            .child(heading(
                "为不同的想法，找一位伙伴。",
                "助手只是不同的对话风格。你可以调整它的说明，或创建自己的伙伴。",
                p,
            ))
            .child(div().grid().grid_cols(2).w_full().gap_3().children(
                self.library.assistants.iter().enumerate().map(|(i, a)| {
                    let selected = i == self.selected_assistant;
                    div()
                        .id(format!("assistant-{}", a.id))
                        .w_full()
                        .min_w_0()
                        .p_4()
                        .rounded_xl()
                        .border_1()
                        .border_color(rgb(if selected { p.accent } else { p.line }))
                        .bg(rgb(if selected { p.soft } else { p.card }))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, w, cx| this.load_assistant(i, w, cx)))
                        .child(icon("sparkles").text_color(rgb(p.accent)))
                        .child(
                            div()
                                .mt_3()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(a.name.clone()),
                        )
                        .child(
                            div()
                                .mt_2()
                                .text_xs()
                                .text_color(rgb(p.muted))
                                .child(a.description.clone()),
                        )
                }),
            ))
            .child(
                card(p)
                    .w_full()
                    .child(field("助手名称", Input::new(&self.assistant_name), p))
                    .child(field(
                        "给助手的说明（系统提示词）",
                        Textarea::new(&self.assistant_prompt),
                        p,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("save-assistant")
                                    .primary()
                                    .label("保存助手")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let name =
                                            this.assistant_name.read(cx).value().trim().to_string();
                                        if name.is_empty() {
                                            this.toast = Some("助手名称不能为空".into());
                                        } else {
                                            let a = &mut this.library.assistants
                                                [this.selected_assistant];
                                            a.name = name;
                                            a.prompt =
                                                this.assistant_prompt.read(cx).value().to_string();
                                            this.persist();
                                            this.toast = Some("助手已保存".into());
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button("use-assistant", "开始对话", "arrow-up-right").on_click(
                                    cx.listener(|this, _, w, cx| {
                                        let aid = this.library.assistants[this.selected_assistant]
                                            .id
                                            .clone();
                                        this.new_chat(w, cx);
                                        if let Some(c) = this.current_mut() {
                                            c.assistant_id = aid;
                                        }
                                        this.persist();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(button("add-assistant", "创建助手", "plus").on_click(
                                cx.listener(|this, _, w, cx| {
                                    this.library.assistants.push(Assistant {
                                        id: id(),
                                        name: "我的助手".into(),
                                        description: "由你定义的对话伙伴".into(),
                                        prompt: String::new(),
                                    });
                                    this.load_assistant(this.library.assistants.len() - 1, w, cx);
                                    this.persist();
                                }),
                            )),
                    ),
            )
    }
    fn render_tools(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        self.render_extensions(p, cx)
    }
    fn render_settings(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let provider = &self.library.providers[self.selected_provider];
        let protocol = provider.protocol.clone();
        div().id("settings-page").size_full().max_w(px(960.)).mx_auto().overflow_y_scroll().p_8().flex().flex_col().gap_6()
        .child(heading("连接你喜欢的模型。","从本地 Ollama 到云端服务，一处管理。API Key 安全保存在系统凭据库。",p))
        .child(div().flex().flex_wrap().gap_2().children(self.library.providers.iter().enumerate().map(|(i,provider)|button(format!("provider-{}",provider.id),provider.name.clone(),if provider.protocol==Protocol::Ollama{"hard-drive"}else{"cloud"}).when(i==self.selected_provider,|b|b.bg(rgb(p.soft)).text_color(rgb(p.accent))).on_click(cx.listener(move|this,_,w,cx|this.load_provider(i,w,cx)))))
            .child(tiny("add-provider","plus","添加兼容服务商").on_click(cx.listener(|this,_,w,cx|{this.library.providers.push(Provider{id:id(),name:"自定义服务商".into(),protocol:Protocol::OpenAi,base_url:"https://api.example.com/v1".into(),models:vec![],enabled:true});this.load_provider(this.library.providers.len()-1,w,cx);this.persist();}))))
        .child(card(p).max_w(px(896.)).child(div().flex().gap_4().child(field("服务商名称",Input::new(&self.provider_name),p).flex_1()).child(field("接口协议",button("protocol",format!("{:?}",protocol),"chevrons-up-down").on_click(cx.listener(|this,_,_,cx|{let p=&mut this.library.providers[this.selected_provider];p.protocol=match p.protocol{Protocol::OpenAi=>Protocol::Ollama,Protocol::Ollama=>Protocol::Anthropic,Protocol::Anthropic=>Protocol::OpenAi};cx.notify();})),p).w(px(200.))))
            .child(field(if protocol==Protocol::Ollama {"API 地址（Ollama 原生：如 http://localhost:11434，无需 /v1）"}else{"API 地址（含 /v1 等服务商前缀）"},Input::new(&self.provider_url),p)).child(field("API Key",Input::new(&self.provider_key),p)).child(field("模型 ID（可手动填写，每行一个）",Textarea::new(&self.provider_models),p))
            .child(div().flex().flex_wrap().gap_2().child(Button::new("save-provider").primary().label("保存配置").on_click(cx.listener(|this,_,w,cx|{this.save_provider(w,cx);})))
                .child(button("fetch-models","连接并获取模型","refresh-cw").on_click(cx.listener(|this,_,w,cx|{if this.save_provider(w,cx){let p=this.library.providers[this.selected_provider].clone();let tx=this.work_tx.clone();this.toast=Some("正在获取模型列表…".into());std::thread::spawn(move||{let result=provider::models(&p);let _=tx.send(Work::Models(p.id,result));});}})))
                .child(button("provider-enabled",if provider.enabled{"已启用"}else{"已停用"},"power").on_click(cx.listener(|this,_,_,cx|{let p=&mut this.library.providers[this.selected_provider];p.enabled= !p.enabled;this.persist();cx.notify();})))
                .child(button("clear-key","移除已保存的密钥","key-round").on_click(cx.listener(|this,_,_,cx|{let p=&this.library.providers[this.selected_provider];this.toast=Some(match storage::set_api_key(p,""){Ok(_)=>"已移除系统凭据库中的密钥".into(),Err(e)=>format!("{e:#}")});cx.notify();}))))
            .child(label("模型能力由服务商和所选模型决定；视觉、PDF、工具、图片生成与语音接口需对应支持。",p.muted)))
        .child(card(p).max_w(px(896.)).child(div().font_weight(FontWeight::SEMIBOLD).child("你的空间")).child(div().flex().flex_wrap().gap_2()
            .child(button("theme",if self.library.settings.dark{"切换浅色外观"}else{"切换深色外观"},"sun-moon").on_click(cx.listener(|this,_,w,cx|{this.library.settings.dark= !this.library.settings.dark;Theme::change(if this.library.settings.dark{ThemeMode::Dark}else{ThemeMode::Light},Some(w),cx);Self::tint(cx,this.library.settings.dark);this.persist();cx.notify();})))
            .child(button("export-library","导出所有会话","download").on_click(cx.listener(|this,_,_,_|this.export(true)))).child(button("import-library","导入会话","upload").on_click(cx.listener(|this,_,_,_|this.import())))
            .child(button("open-storage","打开数据目录","folder-open").on_click(cx.listener(|this,_,_,cx|{if let Some(s)=&this.store{cx.open_url(&format!("file:///{}",s.root.to_string_lossy().replace('\\',"/")));}}))))
            .child(label("Mori 0.1.0 · gpui-kit 0.7.3 · Ctrl+N 新对话 · Ctrl+F 搜索 · Ctrl+Enter 发送 · Esc 停止",p.muted)))
        .when(self.current().is_some(),|d|d.child(button("delete-current","删除当前会话","trash").text_color(rgb(0xb05a48)).disabled(self.generation.is_some()||self.media_busy).on_click(cx.listener(|this,_,w,cx|{if let Some(id)=this.library.active_id.clone(){this.delete_conversation(&id,w,cx);}}))))
    }
}
impl Render for Mori {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.compact =
            window.viewport_size().height < px(760.) || window.viewport_size().width < px(1080.);
        let p = Palette::new(self.library.settings.dark);
        div()
            .id("mori-root")
            .size_full()
            .flex()
            .bg(rgb(p.bg))
            .text_color(rgb(p.ink))
            .font_family("Microsoft YaHei")
            .text_base()
            .on_drop(cx.listener(|this, paths: &ExternalPaths, w, cx| {
                if this.generation.is_none() && !this.media_busy {
                    this.attach_paths(paths.paths().to_vec(), w, cx);
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, w, cx| {
                let key = &event.keystroke.key;
                let ctrl = event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
                if ctrl && key == "n" {
                    this.new_chat(w, cx);
                    cx.stop_propagation();
                } else if ctrl && key == "f" {
                    this.sidebar = true;
                    this.search.update(cx, |s, cx| s.focus(w, cx));
                    cx.notify();
                    cx.stop_propagation();
                } else if ctrl && key == "enter" && this.page == Page::Chat && !this.rename {
                    this.send(w, cx);
                    cx.stop_propagation();
                } else if key == "escape" {
                    if this.generation.is_some() {
                        this.stop(cx);
                    }
                    this.model_picker = false;
                    this.rename = false;
                    cx.notify();
                }
            }))
            .when(self.sidebar, |d| d.child(self.render_sidebar(p, cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .when(!self.sidebar && self.page != Page::Chat, |d| {
                        d.child(
                            button("restore-sidebar", "显示侧栏", "panel-left").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.sidebar = true;
                                    cx.notify();
                                }),
                            ),
                        )
                    })
                    .when_some(self.toast.clone(), |d, text| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .px_6()
                                .py_3()
                                .bg(rgb(p.soft))
                                .border_b_1()
                                .border_color(rgb(p.line))
                                .child(div().text_sm().flex_1().child(text))
                                .when(self.show_undo_delete(), |d| {
                                    d.child(button("undo-delete", "撤销删除", "undo-2").on_click(
                                        cx.listener(|this, _, w, cx| {
                                            this.undo_delete(w, cx);
                                        }),
                                    ))
                                })
                                .child(tiny("dismiss-toast", "x", "关闭提示").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.toast = None;
                                        cx.notify();
                                    }),
                                )),
                        )
                    })
                    .child(match self.page {
                        Page::Chat => self.render_chat(p, cx).into_any_element(),
                        Page::Assistants => self.render_assistants(p, cx).into_any_element(),
                        Page::Tools => self.render_tools(p, cx).into_any_element(),
                        Page::Settings => self.render_settings(p, cx).into_any_element(),
                    }),
            )
    }
}
