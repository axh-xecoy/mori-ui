use super::*;
use gpui_kit::component::{
    input::{Editor, EditorState},
    text::{MarkdownNode, markdown_ast::Node},
};

#[derive(Clone)]
struct LongCode {
    code: SharedString,
    language: SharedString,
    offset: usize,
}
struct CodeBuffer {
    editor: Entity<EditorState>,
    source: SharedString,
}

pub(super) fn parse(
    node: &Node,
    context: &gpui_kit::component::text::MarkdownParseContext<'_>,
) -> Option<MarkdownNode> {
    let Node::Code(block) = node else {
        return None;
    };
    if block.value.len() < 8000 && block.value.lines().take(121).count() <= 120 {
        return None;
    }
    let code: SharedString = block.value.clone().into();
    Some(
        MarkdownNode::new(
            "mori-long-code",
            LongCode {
                code: code.clone(),
                language: block.lang.clone().unwrap_or_else(|| "text".into()).into(),
                offset: block.position.as_ref().map_or(0, |p| p.start.offset),
            },
        )
        .text(code)
        .markdown(context.node_source(node).unwrap_or(&block.value).to_owned()),
    )
}

pub(super) fn render(
    node: &MarkdownNode,
    key: &str,
    tx: mpsc::Sender<Work>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Some(block) = node.data::<LongCode>() else {
        return div().into_any_element();
    };
    let initial = block.clone();
    let buffer = window.use_keyed_state(
        format!("editor-{key}-{}", block.offset),
        cx,
        move |window, cx| {
            let source = initial.code.clone();
            let editor = cx.new(|cx| {
                let mut state = EditorState::new(window, cx)
                    .language(initial.language)
                    .line_number(false)
                    .folding(false);
                state.set_value(initial.code, window, cx);
                state.refresh(cx);
                state
            });
            CodeBuffer { editor, source }
        },
    );
    let state = buffer.read(cx).editor.clone();
    if buffer.read(cx).source != block.code {
        state.update(cx, |state, cx| {
            let offset = state.scroll_offset();
            state.set_value(block.code.clone(), window, cx);
            state.refresh(cx);
            state.set_scroll_offset(offset, cx);
        });
        buffer.update(cx, |buffer, _| buffer.source = block.code.clone());
    }
    #[cfg(feature = "visual-test")]
    super::capture::record_code_editor(key, &state);
    let p = Palette::new(cx.global::<Theme>().mode.is_dark());
    div()
        .id(format!("long-code-{key}-{}", block.offset))
        .min_w_0()
        .w_full()
        .rounded_lg()
        .bg(rgb(p.soft))
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child(div().flex().justify_end().px_3().py_1().child(toolbar(
            block.code.clone(),
            block.language.to_string(),
            tx,
        )))
        .child(
            Editor::new(&state)
                .readonly(true)
                .appearance(false)
                .bordered(false)
                .h(px(280.)),
        )
        .into_any_element()
}

pub(super) fn toolbar(code: SharedString, language: String, tx: mpsc::Sender<Work>) -> Div {
    let download = code.clone();
    let extension = super::views::code_extension(&language);
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(div().text_xs().mr_2().child(language))
        .child(
            tiny("copy-code", "copy", "复制代码")
                .size(px(24.))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()))
                }),
        )
        .child(
            tiny("download-code", "download", "下载代码")
                .size(px(24.))
                .on_click(move |_, _, _| {
                    let code = download.clone();
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title(tr("保存代码"))
                            .set_file_name(format!("snippet.{extension}"))
                            .add_filter(tr("代码文件"), &[extension])
                            .save_file()
                        {
                            let result = std::fs::write(path, code.as_bytes())
                                .map(|_| "代码已保存".into())
                                .map_err(anyhow::Error::from);
                            let _ = tx.send(Work::Notice(result));
                        }
                    });
                }),
        )
}
