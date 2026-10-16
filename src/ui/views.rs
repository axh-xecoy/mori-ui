use super::*;
use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};

#[derive(IntoElement)]
struct ToolPreviewPanel {
    key: String,
    preview: String,
    height: f32,
}

impl RenderOnce for ToolPreviewPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = window
            .use_keyed_state(self.key.clone(), cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        #[cfg(feature = "visual-test")]
        super::capture::record_tool_scroll(&self.key, &handle);
        div()
            .relative()
            .min_w_0()
            // Contain wheel events even when the inner scroller reaches an edge.
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id(self.key.clone())
                    .min_w_0()
                    .max_h(px(self.height))
                    .overflow_scroll()
                    .track_scroll(&handle)
                    .child(
                        TextView::markdown(format!("text-{}", self.key), self.preview)
                            .selectable(true),
                    ),
            )
            .child(
                div().absolute().inset_0().child(
                    Scrollbar::new(&handle)
                        .id(format!("bar-{}", self.key))
                        .mode(ScrollbarMode::Always)
                        .viewport_from_layout(),
                ),
            )
    }
}

fn tool_preview_panel(key: String, text: &str, language: &str, height: f32) -> impl IntoElement {
    ToolPreviewPanel {
        key,
        preview: tool_output_preview(text, language),
        height,
    }
}

fn tool_output_preview(text: &str, language: &str) -> String {
    let preview = text.chars().take(20000).collect::<String>();
    // Keep returned Markdown/backticks literal inside the output panel.
    let run = preview.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(run.max(2) + 1);
    format!("{fence}{language}\n{preview}\n{fence}")
}

pub(super) fn code_extension(language: &str) -> &'static str {
    match language
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "python" | "py" => "py",
        "rust" | "rs" => "rs",
        "javascript" | "js" | "jsx" => "js",
        "typescript" | "ts" => "ts",
        "tsx" => "tsx",
        "bash" | "sh" | "shell" => "sh",
        "powershell" | "ps1" => "ps1",
        "json" => "json",
        "html" => "html",
        "css" => "css",
        "go" => "go",
        "c" => "c",
        "cpp" | "c++" => "cpp",
        "sql" => "sql",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "java" => "java",
        "markdown" | "md" => "md",
        _ => "txt",
    }
}

impl Mori {
    fn rich_markdown(
        &self,
        id: impl Into<ElementId>,
        source: impl AsRef<str>,
        color: u32,
    ) -> TextView {
        use gpui_kit::component::{highlighter::HighlightTheme, text::TextViewStyle};
        let tx = self.work_tx.clone();
        let dark = self.library.settings.dark;
        let id: ElementId = id.into();
        let long_document = source.as_ref().len() > 32000
            && source
                .as_ref()
                .split("```")
                .step_by(2)
                .map(str::len)
                .sum::<usize>()
                > 32000;
        let code_key = format!("{id:?}");
        let code_tx = tx.clone();
        let (_, prepared) = self.text_cache.borrow_mut().get(
            format!("markdown-{id:?}"),
            source.as_ref(),
            crate::math::normalize,
        );
        let mut block_style = StyleRefinement::default().pt(px(40.)).max_h(px(320.));
        block_style.overflow.y = Some(Overflow::Scroll);
        crate::math::markdown_prepared(id, prepared, color)
            .style(
                TextViewStyle {
                    highlight_theme: if dark {
                        HighlightTheme::default_dark()
                    } else {
                        HighlightTheme::default_light()
                    },
                    is_dark: dark,
                    ..Default::default()
                }
                .code_block(block_style),
            )
            .markdown_block_parser(super::long_code::parse)
            .markdown_block_renderer("mori-long-code", move |node, window, cx| {
                super::long_code::render(node, &code_key, code_tx.clone(), window, cx)
            })
            .when(long_document, |view| view.scrollable(true).h(px(480.)))
            .code_block_actions(move |block, _, _| {
                super::long_code::toolbar(
                    block.code(),
                    block.lang().unwrap_or_else(|| "text".into()).to_string(),
                    tx.clone(),
                )
            })
    }

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
            .w(px(self.library.settings.sidebar_width.clamp(220., 380.)))
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
                    .label(tr("新的对话"))
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
                                                            tr("选择一个模型")
                                                        } else {
                                                            self.library
                                                                .providers
                                                                .iter()
                                                                .find(|p| p.id == c.provider_id)
                                                                .map(|p| p.model_name(&c.model))
                                                                .unwrap_or_else(|| c.model.clone())
                                                        }
                                                    )),
                                            ),
                                    )
                                    .child({
                                        let view = cx.entity().downgrade();
                                        let locked = self
                                            .generation
                                            .as_ref()
                                            .is_some_and(|g| g.conversation == c.id)
                                            || self.media_busy;
                                        Button::new(format!("conversation-menu-{}", c.id))
                                            .ghost()
                                            .icon(icon("ellipsis"))
                                            .size(px(28.))
                                            .text_color(rgb(p.muted))
                                            .on_click(|_, _, cx| cx.stop_propagation())
                                            .dropdown_menu(move |menu, _, _| {
                                                let rename_view = view.clone();
                                                let rename_id = rename_id.clone();
                                                let delete_view = view.clone();
                                                let delete_id = delete_id.clone();
                                                menu.item(
                                                    PopupMenuItem::new(tr("重命名"))
                                                        .icon(icon("pencil"))
                                                        .on_click(move |_, w, cx| {
                                                            let _ = rename_view.update(
                                                                cx,
                                                                |this, cx| {
                                                                    this.rename_conversation(
                                                                        rename_id.clone(),
                                                                        w,
                                                                        cx,
                                                                    )
                                                                },
                                                            );
                                                        }),
                                                )
                                                .separator()
                                                .item(
                                                    PopupMenuItem::new(tr("删除会话"))
                                                        .icon(icon("trash"))
                                                        .disabled(locked)
                                                        .on_click(move |_, w, cx| {
                                                            let _ = delete_view.update(
                                                                cx,
                                                                |this, cx| {
                                                                    this.delete_conversation(
                                                                        &delete_id, w, cx,
                                                                    )
                                                                },
                                                            );
                                                        }),
                                                )
                                            })
                                    }),
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
                                    .child(tr("今天，让想法慢慢生长。")),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(p.muted))
                                    .child(tr("一个问题、一张图片，或一个还没成形的灵感。")),
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
                                    this.composer
                                        .update(cx, |s, cx| s.set_value(tr(prompt), w, cx));
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
                                                .child(tr(title)),
                                        ),
                                )
                                .child(
                                    div()
                                        .mt_2()
                                        .text_xs()
                                        .text_color(rgb(p.muted))
                                        .child(tr(desc)),
                                )
                        }),
                    ))
                    .when(!compact && self.attachments.is_empty(), |d| {
                        d.child(label("支持本地模型，也连接你喜欢的 AI 服务", p.muted))
                    }),
            )
    }
    fn render_message(
        &self,
        m: &Message,
        index: usize,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        #[cfg(feature = "visual-test")]
        super::capture::record_message_row();
        let user = m.role == "user";
        // A selectable TextView has no intrinsic width through the extra bubble
        // wrapper. Size short user bubbles with the actual font, then let flex
        // shrink long messages to the available viewport width.
        let user_width = if !user || !m.attachments.is_empty() || m.content.len() > 600 {
            px(680.)
        } else {
            m.content
                .lines()
                .map(|line| {
                    let run = TextRun {
                        len: line.len(),
                        font: font("Microsoft YaHei"),
                        color: rgb(p.ink).into(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    window
                        .text_system()
                        .shape_line(line.to_string().into(), px(16.), &[run], None)
                        .width
                        + px(40.)
                })
                .fold(px(72.), |width, line| width.max(line))
                .min(px(680.))
        };
        let (text, _) =
            self.text_cache
                .borrow_mut()
                .get(format!("source-{}", m.id), &m.content, str::to_owned);
        let speak_text = text.clone();
        let expanded = self.expanded.contains(&m.id);
        let reasoning_id = m.id.clone();
        div()
            .flex()
            .when(user, |d| d.flex_row_reverse())
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
                    .when(user, |d| {
                        d.flex_initial().w(user_width).max_w(px(680.)).gap_1()
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .min_w_0()
                            .when(user, |d| d.p_4().rounded_xl().bg(rgb(p.soft)))
                            .when(!user, |d| {
                                d.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(self.reply_model_name(m)),
                                        )
                                        .when(
                                            !user
                                                && m.elapsed_ms > 0
                                                && !self.generation.as_ref().is_some_and(|g| {
                                                    self.current()
                                                        .is_some_and(|c| c.id == g.conversation)
                                                        && self
                                                            .current()
                                                            .and_then(|c| c.messages.last())
                                                            .is_some_and(|last| last.id == m.id)
                                                }),
                                            |d| {
                                                d.child(label(
                                                    format!(
                                                        "{:.1}s · {} tokens",
                                                        m.elapsed_ms as f64 / 1000.,
                                                        m.tokens
                                                    ),
                                                    p.muted,
                                                ))
                                            },
                                        ),
                                )
                            })
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
                                            .w_full()
                                            .justify_start()
                                            .h(px(36.))
                                            .px_3()
                                            .bg(rgb(p.card))
                                            .border_1()
                                            .border_color(rgb(p.line))
                                            .child(div().flex_1())
                                            .child(
                                                icon(if expanded {
                                                    "chevron-down"
                                                } else {
                                                    "chevron-right"
                                                })
                                                .text_color(rgb(p.muted)),
                                            )
                                            .on_click(
                                                cx.listener(move |this, _, _, cx| {
                                                    if !this.expanded.remove(&reasoning_id) {
                                                        this.expanded.insert(reasoning_id.clone());
                                                    }
                                                    cx.notify();
                                                }),
                                            ),
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
                                                        self.rich_markdown(
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
                            .when(!m.context_note.is_empty(), |d| {
                                d.child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(p.muted))
                                        .child(m.context_note.clone()),
                                )
                            })
                            .when(!m.content.is_empty(), |d| {
                                let content = if user {
                                    TextView::html(
                                        format!("md-{}", m.id),
                                        self.text_cache
                                            .borrow_mut()
                                            .get(
                                                format!("user-{}", m.id),
                                                &m.content,
                                                user_message_html,
                                            )
                                            .1,
                                    )
                                    .style(
                                        gpui_kit::component::text::TextViewStyle {
                                            paragraph_gap: rems(0.),
                                            ..Default::default()
                                        },
                                    )
                                } else {
                                    self.rich_markdown(format!("md-{}", m.id), &m.content, p.ink)
                                };
                                d.child(content.selectable(true).w_full())
                            })
                            .children(m.attachments.iter().enumerate().map(|(i, a)| {
                                let path = a.path.clone();
                                let file_path = path.clone();
                                let is_image = a.mime.starts_with("image/");
                                div()
                                    .id(format!("attachment-{}-{i}", m.id))
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .when(is_image, |d| {
                                        let (width, height) = image_preview_size(&path);
                                        d.child(
                                            div()
                                                .id(format!("attachment-image-{}-{i}", m.id))
                                                .relative()
                                                .w(px(width))
                                                .h(px(height))
                                                .cursor_pointer()
                                                .on_click(move |_, _, cx| {
                                                    cx.open_url(&format!(
                                                        "file:///{}",
                                                        path.replace('\\', "/")
                                                    ))
                                                })
                                                .child(
                                                    img(std::path::PathBuf::from(&a.path))
                                                        .w_full()
                                                        .h_full()
                                                        .object_fit(ObjectFit::Contain)
                                                        .rounded_lg(),
                                                ),
                                        )
                                    })
                                    .when(!is_image, |d| {
                                        d.child(
                                            button(
                                                format!("attachment-file-{}-{i}", m.id),
                                                a.name.clone(),
                                                "file-text",
                                            )
                                            .on_click(
                                                move |_, _, cx| {
                                                    cx.open_url(&format!(
                                                        "file:///{}",
                                                        file_path.replace('\\', "/")
                                                    ))
                                                },
                                            ),
                                        )
                                    })
                            }))
                            .children(m.tools.iter().map(|t| {
                                let key = format!("tool-{}-{}", m.id, t.id);
                                let expanded = self.expanded.contains(&key);
                                let toggle_key = key.clone();
                                let command =
                                    serde_json::from_str::<serde_json::Value>(&t.arguments)
                                        .ok()
                                        .and_then(|value| {
                                            value["command"].as_str().map(str::to_string)
                                        })
                                        .unwrap_or_default();
                                let summary = command
                                    .lines()
                                    .next()
                                    .unwrap_or("")
                                    .chars()
                                    .take(100)
                                    .collect::<String>();
                                let waiting = self
                                    .approval
                                    .as_ref()
                                    .is_some_and(|approval| approval.call.id == t.id);
                                let status = if t.result.is_some() {
                                    "已返回"
                                } else if waiting {
                                    "待确认"
                                } else if self.generation.as_ref().is_some_and(|g| {
                                    self.current().is_some_and(|c| c.id == g.conversation)
                                }) {
                                    "调用中"
                                } else {
                                    "未完成"
                                };
                                card(p)
                                    .gap_2()
                                    .p_3()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .id(format!("toggle-{key}"))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .min_w_0()
                                            .cursor_pointer()
                                            .hover(|d| d.text_color(rgb(p.accent)))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if !this.expanded.remove(&toggle_key) {
                                                    this.expanded.insert(toggle_key.clone());
                                                }
                                                cx.notify();
                                            }))
                                            .child(
                                                icon(if t.name == "bash" {
                                                    "terminal"
                                                } else {
                                                    "blocks"
                                                })
                                                .size(px(16.))
                                                .flex_shrink_0()
                                                .text_color(rgb(p.accent)),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .flex_shrink_0()
                                                    .child(if t.name == "bash" {
                                                        "Bash".to_string()
                                                    } else {
                                                        t.name.clone()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_xs()
                                                    .text_color(rgb(p.muted))
                                                    .truncate()
                                                    .child(summary),
                                            )
                                            .child(label(status, p.muted).flex_shrink_0())
                                            .child(
                                                icon(if expanded {
                                                    "chevron-up"
                                                } else {
                                                    "chevron-down"
                                                })
                                                .size(px(16.))
                                                .flex_shrink_0(),
                                            ),
                                    )
                                    .when(expanded, |d| {
                                        let args =
                                            serde_json::from_str::<serde_json::Value>(&t.arguments)
                                                .ok()
                                                .and_then(|value| {
                                                    serde_json::to_string_pretty(&value).ok()
                                                })
                                                .unwrap_or_else(|| t.arguments.clone());
                                        let arguments = t.arguments.clone();
                                        d.child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .child(label("调用参数", p.muted))
                                                .child(
                                                    tiny(
                                                        format!("copy-args-{key}"),
                                                        "copy",
                                                        "复制调用参数",
                                                    )
                                                    .on_click(move |_, _, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(
                                                                arguments.clone(),
                                                            ),
                                                        )
                                                    }),
                                                ),
                                        )
                                        .child(tool_preview_panel(
                                            format!("args-scroll-{key}"),
                                            &args,
                                            "json",
                                            120.,
                                        ))
                                        .when_some(
                                            t.result.as_ref(),
                                            |d, result| {
                                                let output = result.clone();
                                                let count = result.lines().count();
                                                let count_label =
                                                    if self.library.settings.language == "en" {
                                                        format!("{count} lines")
                                                    } else {
                                                        format!("{count} 行")
                                                    };
                                                let clipped = result.chars().count() > 20000;
                                                d.child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .gap_2()
                                                                .child(label("返回结果", p.muted))
                                                                .child(label(count_label, p.muted)),
                                                        )
                                                        .child(
                                                            tiny(
                                                                format!("copy-result-{key}"),
                                                                "copy",
                                                                "复制完整已保存结果",
                                                            )
                                                            .on_click(move |_, _, cx| {
                                                                cx.write_to_clipboard(
                                                                    ClipboardItem::new_string(
                                                                        output.clone(),
                                                                    ),
                                                                )
                                                            }),
                                                        ),
                                                )
                                                .child(tool_preview_panel(
                                                    format!("output-scroll-{key}"),
                                                    result,
                                                    "text",
                                                    240.,
                                                ))
                                                .when(clipped, |d| {
                                                    d.child(label(
                                                    "仅预览前 20,000 字符，可复制完整已保存结果",
                                                    p.muted,
                                                ))
                                                })
                                            },
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
                                        .child(tr(&e)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .when(user, |d| d.justify_end())
                            .child(tiny(format!("copy-{}", m.id), "copy", "复制消息").on_click(
                                move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        text.to_string(),
                                    ))
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
                                        move |this, _, _, cx| {
                                            this.speak(speak_text.to_string(), cx)
                                        },
                                    )),
                                )
                            })
                            .when(
                                !user
                                    && self
                                        .current()
                                        .and_then(|c| c.messages.last())
                                        .is_some_and(|last| last.id == m.id),
                                |d| {
                                    d.child(
                                        tiny(format!("retry-{}", m.id), "refresh-cw", "重新生成")
                                            .disabled(self.generation.is_some() || self.media_busy)
                                            .on_click(cx.listener(|this, _, _, cx| this.retry(cx))),
                                    )
                                },
                            ),
                    ),
            )
    }
    fn render_composer(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let busy = self.generation.is_some() || self.media_busy;
        let tools = self.library.settings.tools_enabled;
        let paste_view = cx.entity().downgrade();
        div()
            .w_full()
            .flex_shrink_0()
            .min_w_0()
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
                                        .label(tr("允许这一次"))
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
                    .flex_shrink_0()
                    .min_w_0()
                    .w_full()
                    .max_w(px(896.))
                    .mx_auto()
                    .p_3()
                    .rounded_xl()
                    .bg(rgb(p.card))
                    .border_1()
                    .border_color(rgb(p.line))
                    .id("composer-drop-zone")
                    .drag_over::<ExternalPaths>(move |style, _, _, _| {
                        style.bg(rgb(p.soft)).border_color(rgb(p.accent))
                    })
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, w, cx| {
                        if this.generation.is_none() && !this.media_busy {
                            this.attach_paths(paths.paths().to_vec(), w, cx);
                        }
                        cx.stop_propagation();
                    }))
                    .when(!self.attachments.is_empty(), |d| {
                        d.child(
                            div()
                                .id("composer-attachments")
                                .flex()
                                .gap_2()
                                .w_full()
                                .overflow_x_scroll()
                                .pb_2()
                                .mb_2()
                                .children(self.attachments.iter().enumerate().map(|(i, a)| {
                                    let path = a.path.clone();
                                    if a.mime.starts_with("image/") {
                                        div()
                                            .relative()
                                            .w(px(96.))
                                            .flex_shrink_0()
                                            .child(
                                                img(std::path::PathBuf::from(&a.path))
                                                    .size(px(96.))
                                                    .object_fit(ObjectFit::Cover)
                                                    .rounded_lg(),
                                            )
                                            .child(
                                                div()
                                                    .absolute()
                                                    .top_1()
                                                    .right_1()
                                                    .bg(rgb(p.card))
                                                    .rounded_full()
                                                    .child(
                                                        tiny(
                                                            format!("remove-image-{i}"),
                                                            "x",
                                                            "移除附件",
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.remove_attachment(&path, cx)
                                                            },
                                                        )),
                                                    ),
                                            )
                                            .into_any_element()
                                    } else {
                                        button(format!("pending-{i}"), a.name.clone(), "file-text")
                                            .bg(rgb(p.soft))
                                            .max_w(px(260.))
                                            .flex_shrink_0()
                                            .overflow_hidden()
                                            .tooltip(a.name.clone())
                                            .child(icon("x"))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.remove_attachment(&path, cx)
                                            }))
                                            .into_any_element()
                                    }
                                })),
                        )
                    })
                    .child(
                        Textarea::new(&self.composer)
                            .on_paste(move |item, window, cx| {
                                let media = item.entries.iter().any(|entry| match entry {
                                    ClipboardEntry::Image(_) => true,
                                    ClipboardEntry::ExternalPaths(paths) => {
                                        !paths.paths().is_empty()
                                    }
                                    ClipboardEntry::String(_) => false,
                                });
                                if !media {
                                    return false;
                                }
                                let item = item.clone();
                                let view = paste_view.clone();
                                let handle = window.window_handle();
                                // Input owns its state during paste; defer draft updates until that borrow ends.
                                cx.defer(move |cx| {
                                    let _ = handle.update(cx, |_, window, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            this.paste_attachments(item, window, cx)
                                        });
                                    });
                                });
                                true
                            })
                            .appearance(false)
                            .bordered(false)
                            .w_full(),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .min_w_0()
                            .gap_2()
                            .mt_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .flex_1()
                                    .min_w_0()
                                    .flex_wrap()
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
                                    .child({
                                        let view = cx.entity().downgrade();
                                        let mode = self.library.settings.thinking;
                                        button(
                                            "thinking-mode",
                                            match mode {
                                                None => "思考：自动",
                                                Some(true) => "思考：开启",
                                                Some(false) => "思考：关闭",
                                            },
                                            "brain",
                                        )
                                        .disabled(busy || self.image_mode)
                                        .dropdown_menu(
                                            move |mut menu, _, _| {
                                                for (value, title) in [
                                                    (None, "思考：自动"),
                                                    (Some(true), "思考：开启"),
                                                    (Some(false), "思考：关闭"),
                                                ] {
                                                    let view = view.clone();
                                                    menu = menu.item(
                                                        PopupMenuItem::new(tr(title))
                                                            .checked(mode == value)
                                                            .on_click(move |_, _, cx| {
                                                                let _ =
                                                                    view.update(cx, |this, cx| {
                                                                        this.library
                                                                            .settings
                                                                            .thinking = value;
                                                                        this.persist();
                                                                        cx.notify();
                                                                    });
                                                            }),
                                                    );
                                                }
                                                menu
                                            },
                                        )
                                    })
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
                                    .flex_shrink_0()
                                    .ml_auto()
                                    .when(!self.compact && !self.model_picker, |d| {
                                        d.child(label(
                                            if self.library.settings.send_on_enter {
                                                "Enter 发送 · Shift+Enter 换行"
                                            } else {
                                                "Ctrl+Enter 发送 · Enter 换行"
                                            },
                                            p.muted,
                                        ))
                                    })
                                    .child(if self.generation.is_some() {
                                        Button::new("send")
                                            .primary()
                                            .icon(icon("square"))
                                            .small()
                                            .label(tr("停止"))
                                            .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
                                    } else {
                                        Button::new("send")
                                            .primary()
                                            .icon(icon("arrow-up"))
                                            .small()
                                            .label(tr("发送"))
                                            .disabled(
                                                self.media_busy || self.attachments_loading > 0,
                                            )
                                            .on_click(
                                                cx.listener(|this, _, w, cx| this.send(w, cx)),
                                            )
                                    }),
                            ),
                    ),
            )
            .child(div().h(px(20.)).text_center().child(label(
                if self.generation.is_some() {
                    String::new()
                } else if self.attachments_loading > 0 {
                    tr("正在添加附件…")
                } else {
                    self.status.clone()
                },
                p.muted,
            )))
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
    fn render_chat(&self, p: Palette, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let c = self.current();
        let title = c
            .map(|c| c.title.as_str())
            .unwrap_or("新的对话")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let model = c
            .filter(|c| !c.model.is_empty())
            .map(|c| {
                self.library
                    .providers
                    .iter()
                    .find(|p| p.id == c.provider_id)
                    .map(|p| p.model_name(&c.model))
                    .unwrap_or_else(|| c.model.clone())
            })
            .unwrap_or_else(|| tr("选择模型"));
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
                            .flex_1()
                            .min_w_0()
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
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_sm()
                                            .truncate()
                                            .child(title),
                                    )
                                    .child(
                                        label(
                                            assistant
                                                .split_whitespace()
                                                .collect::<Vec<_>>()
                                                .join(" "),
                                            p.muted,
                                        )
                                        .truncate(),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .flex_shrink_0()
                            .ml_3()
                            .child(
                                button("model-picker", model.to_string(), "chevrons-up-down")
                                    .max_w(px(240.))
                                    .overflow_hidden()
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
                                let conversation = c.unwrap();
                                self.scroll.sync(conversation, &self.expanded);
                                let view = cx.entity().downgrade();
                                div()
                                    .id("messages")
                                    .flex_1()
                                    .min_h_0()
                                    .child(
                                        list(self.scroll.state.clone(), move |i, window, cx| {
                                            view.update(cx, |this, cx| {
                                                let Some(message) =
                                                    this.current().and_then(|c| c.messages.get(i))
                                                else {
                                                    return div().into_any_element();
                                                };
                                                div()
                                                    .px_8()
                                                    .child(div().max_w(px(896.)).mx_auto().child(
                                                        this.render_message(
                                                            message, i, p, window, cx,
                                                        ),
                                                    ))
                                                    .into_any_element()
                                            })
                                            .unwrap_or_else(|_| div().into_any_element())
                                        })
                                        .size_full(),
                                    )
                                    .into_any_element()
                            })
                            .child(
                                div().w_full().flex_shrink_0().px_8().child(
                                    div()
                                        .w_full()
                                        .max_w(px(896.))
                                        .mx_auto()
                                        .h(px(24.))
                                        .flex()
                                        .items_center()
                                        .text_sm()
                                        .text_color(rgb(p.accent))
                                        .when(
                                            self.generation.as_ref().is_some_and(|g| {
                                                self.library.active_id.as_ref()
                                                    == Some(&g.conversation)
                                            }) && self.approval.is_none(),
                                            |d| d.child(tr("●  Mori 正在回复…")),
                                        ),
                                ),
                            )
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
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("选择模型")),
                    )
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
                                let shown = provider.model_name(&model);
                                navigation_button(format!("model-{pid}-{model}"), shown, "cpu")
                                    .tooltip(model.clone())
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
                button("configure-models", "管理连接", "settings-2").on_click(cx.listener(
                    |this, _, w, cx| {
                        this.settings_tab = SettingsTab::Connections;
                        this.navigate(Page::Settings, w, cx);
                    },
                )),
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
                                .child(tr(&a.name)),
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
                                    .label(tr("保存助手"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let name =
                                            this.assistant_name.read(cx).value().trim().to_string();
                                        if name.is_empty() {
                                            this.show_toast("助手名称不能为空".into());
                                        } else {
                                            let a = &mut this.library.assistants
                                                [this.selected_assistant];
                                            a.name = name;
                                            a.prompt =
                                                this.assistant_prompt.read(cx).value().to_string();
                                            this.persist();
                                            this.show_toast("助手已保存".into());
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button("use-assistant", "开始对话", "arrow-up-right").on_click(
                                    cx.listener(|this, _, w, cx| {
                                        this.new_chat(w, cx);
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
}
impl Render for Mori {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.compact =
            window.viewport_size().height < px(760.) || window.viewport_size().width < px(1080.);
        let p = Palette::new(self.library.settings.dark);
        let show_sidebar = self.sidebar
            && !(self.page == Page::Chat
                && self.model_picker
                && f32::from(window.viewport_size().width)
                    - self.library.settings.sidebar_width.clamp(220., 380.)
                    - 290.
                    < 420.);
        div()
            .id("mori-root")
            .relative()
            .size_full()
            .flex()
            .bg(rgb(p.bg))
            .text_color(rgb(p.ink))
            .font_family("Microsoft YaHei")
            .text_base()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    if show_sidebar
                        && (f32::from(event.position.x) - this.library.settings.sidebar_width).abs()
                            <= 4.
                    {
                        this.resizing_sidebar = true;
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, w, cx| {
                if this.resizing_sidebar {
                    let max = (f32::from(w.viewport_size().width) - 500.).clamp(220., 380.);
                    this.library.settings.sidebar_width =
                        f32::from(event.position.x).clamp(220., max);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.resizing_sidebar {
                        this.resizing_sidebar = false;
                        this.persist();
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.resizing_sidebar {
                        this.resizing_sidebar = false;
                        this.persist();
                        cx.notify();
                    }
                }),
            )
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
                } else if key == "escape" {
                    this.resizing_sidebar = false;
                    if this.generation.is_some() {
                        this.stop(cx);
                    }
                    this.model_picker = false;
                    this.rename = false;
                    cx.notify();
                }
            }))
            .when(show_sidebar, |d| d.child(self.render_sidebar(p, cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .relative()
                    .overflow_hidden()
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
                    .child(match self.page {
                        Page::Chat => self.render_chat(p, window, cx).into_any_element(),
                        Page::Assistants => self.render_assistants(p, cx).into_any_element(),
                        Page::Tools => self.render_tools(p, cx).into_any_element(),
                        Page::Settings => self.render_settings(p, cx).into_any_element(),
                    })
                    .when_some(self.toast.clone(), |d, text| {
                        // Animate only the visual child. The stationary hitbox
                        // must cover the header until the exit animation ends.
                        d.child(
                            div()
                                .id("toast-hit-shield")
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .occlude()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .child(
                                    div()
                                        .relative()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .px_6()
                                        .py_3()
                                        .bg(rgb(p.soft))
                                        .border_b_1()
                                        .border_color(rgb(p.line))
                                        .child(div().text_sm().flex_1().child(tr(&text)))
                                        .when(self.show_undo_delete(), |d| {
                                            d.child(
                                                button("undo-delete", "撤销删除", "undo-2")
                                                    .on_click(cx.listener(|this, _, w, cx| {
                                                        this.undo_delete(w, cx);
                                                    })),
                                            )
                                        })
                                        .child(tiny("dismiss-toast", "x", "关闭提示").on_click(
                                            cx.listener(|this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.dismiss_toast();
                                                cx.notify();
                                            }),
                                        ))
                                        .with_animation(
                                            format!(
                                                "notice-{}-{}",
                                                self.toast_revision,
                                                self.toast_closing.is_some()
                                            ),
                                            Animation::new(Duration::from_millis(
                                                if self.toast_closing.is_some() {
                                                    300
                                                } else {
                                                    180
                                                },
                                            ))
                                            .with_easing(|t: f32| 1. - (1. - t).powi(3)),
                                            {
                                                let closing = self.toast_closing.is_some();
                                                move |d, progress| {
                                                    if closing {
                                                        d.top(px(-48. * progress))
                                                            .opacity(1. - progress)
                                                    } else {
                                                        d.top(px(-24. * (1. - progress)))
                                                            .opacity(progress)
                                                    }
                                                }
                                            },
                                        ),
                                ),
                        )
                    }),
            )
            .when(show_sidebar, |d| {
                d.child(
                    div()
                        .id("sidebar-resize")
                        .absolute()
                        .h_full()
                        .left(px(
                            self.library.settings.sidebar_width.clamp(220., 380.) - 2.5
                        ))
                        .top_0()
                        .bottom_0()
                        .w(px(5.))
                        .cursor(CursorStyle::ResizeLeftRight)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.resizing_sidebar = true;
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                )
            })
    }
}
