use gpui_kit::component::text::{
    InlineElement, InlineRenderContext, MarkdownExtensions, MarkdownNode, MarkdownParseContext,
    MarkdownPlugin, TextView, markdown_ast::Node,
};
use gpui_kit::{
    App, ElementId, Image, ImageFormat, IntoElement, ParentElement, Styled, Window, div, img, px,
    rgb,
};
use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_types::{color::Color, math_style::MathStyle};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone)]
struct Formula {
    image: Arc<Image>,
    width: f32,
    height: f32,
    baseline: f32,
}
type CacheKey = (String, bool, u32);
type FormulaCache = HashMap<CacheKey, Option<Formula>>;

fn prepare(source: &str, display: bool, color: u32) -> Option<Formula> {
    static CACHE: OnceLock<Mutex<FormulaCache>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    let key = (source.to_string(), display, color);
    if let Some(result) = cache.lock().ok()?.get(&key) {
        return result.clone();
    }
    let result = (|| {
        if source.len() > 8192 {
            return None;
        }
        let ast = ratex_parser::parse(source).ok()?;
        let ink = Color::new(
            ((color >> 16) & 255) as f32 / 255.,
            ((color >> 8) & 255) as f32 / 255.,
            (color & 255) as f32 / 255.,
            1.,
        );
        let options = LayoutOptions::default()
            .with_style(if display {
                MathStyle::Display
            } else {
                MathStyle::Text
            })
            .with_color(ink);
        let list = to_display_list(&layout(&ast, &options));
        let font_size = if display { 18. } else { 16. };
        let width = (list.width as f32 * font_size + 4.).ceil();
        let height = ((list.height + list.depth) as f32 * font_size + 4.).ceil();
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.
            || height <= 0.
            || width > 2048.
            || height > 1024.
        {
            return None;
        }
        let png = ratex_render::render_to_png(
            &list,
            &ratex_render::RenderOptions {
                font_size,
                padding: 2.,
                background_color: Color::new(0., 0., 0., 0.),
                device_pixel_ratio: 2.,
                ..Default::default()
            },
        )
        .ok()?;
        Some(Formula {
            image: Arc::new(Image::from_bytes(ImageFormat::Png, png)),
            width,
            height,
            baseline: list.height as f32 * font_size + 2.,
        })
    })();
    let mut entries = cache.lock().ok()?;
    if entries.len() >= 128 {
        entries.clear();
    }
    entries.insert(key, result.clone());
    result
}

struct MathPlugin {
    block: bool,
    color: u32,
}
impl MarkdownPlugin for MathPlugin {
    fn is_block(&self) -> bool {
        self.block
    }
    fn name(&self) -> &str {
        if self.block {
            "mori-math-block"
        } else {
            "mori-math-inline"
        }
    }
    fn parse(&self, node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let value = match (self.block, node) {
            (true, Node::Math(m)) => &m.value,
            (false, Node::InlineMath(m)) => &m.value,
            _ => return None,
        };
        let source = cx.node_source(node).unwrap_or(value);
        // Formula resources are prepared in the Markdown parsing task, never from paint.
        Some(
            MarkdownNode::new(self.name(), prepare(value, self.block, self.color))
                .text(source.to_string())
                .markdown(source.to_string())
                .accessibility_label(value.to_string()),
        )
    }
    fn render(&self, node: &MarkdownNode, window: &mut Window, _: &mut App) -> impl IntoElement {
        if let Some(Some(formula)) = node.data::<Option<Formula>>() {
            let available = (f32::from(window.viewport_size().width) - 400.).clamp(180., 780.);
            let scale = (available / formula.width).min(1.);
            div()
                .w_full()
                .flex()
                .justify_center()
                .py_2()
                .child(
                    img(formula.image.clone())
                        .w(px(formula.width * scale))
                        .h(px(formula.height * scale)),
                )
                .into_any_element()
        } else {
            div()
                .text_color(rgb(self.color))
                .child(node.as_text().to_string())
                .into_any_element()
        }
    }
    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        _: &mut Window,
        _: &mut App,
    ) -> Option<InlineElement> {
        let formula = node.data::<Option<Formula>>()?.as_ref()?;
        let scale = f32::from(context.font_size()) / 16.;
        Some(
            InlineElement::new(
                img(formula.image.clone())
                    .w(px(formula.width * scale))
                    .h(px(formula.height * scale)),
            )
            .with_baseline(px(formula.baseline * scale)),
        )
    }
}

/// Normalize model conventions without touching fenced code or inline code.
pub fn normalize(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut i = 0;
    let mut fence: Option<(u8, usize)> = None;
    let mut ticks = 0;
    while i < source.len() {
        let rest = &source[i..];
        let byte = source.as_bytes()[i];
        if byte == b'`' || byte == b'~' {
            let run = rest.bytes().take_while(|b| *b == byte).count();
            let line = source[..i].rsplit('\n').next().unwrap_or("");
            if run >= 3 && line.len() <= 3 && line.trim().is_empty() {
                match fence {
                    None => fence = Some((byte, run)),
                    Some((c, n)) if c == byte && run >= n => fence = None,
                    _ => {}
                }
            } else if fence.is_none() && byte == b'`' {
                if ticks == 0 {
                    ticks = run;
                } else if ticks == run {
                    ticks = 0;
                }
            }
            result.push_str(&rest[..run]);
            i += run;
            continue;
        }
        if fence.is_none() && ticks == 0 {
            let pair = if rest.starts_with("$$") {
                Some(("$$", "$$", true))
            } else if rest.starts_with(r"\[") {
                Some((r"\[", r"\]", true))
            } else if rest.starts_with(r"\(") {
                Some((r"\(", r"\)", false))
            } else {
                None
            };
            if let Some((open, close, display)) = pair
                && let Some(end) = rest[open.len()..].find(close)
            {
                let content = &rest[open.len()..open.len() + end];
                if display {
                    result.push_str("\n\n$$\n");
                    result.push_str(content.trim());
                    result.push_str("\n$$\n\n");
                } else {
                    result.push('$');
                    result.push_str(content);
                    result.push('$');
                }
                i += open.len() + end + close.len();
                continue;
            }
            if rest.starts_with(r"\$") || rest.starts_with(r"\\") {
                result.push_str(&rest[..2]);
                i += 2;
                continue;
            }
        }
        let ch = rest.chars().next().unwrap();
        result.push(ch);
        i += ch.len_utf8();
    }
    result
}

pub fn markdown_prepared(
    id: impl Into<ElementId>,
    source: impl Into<gpui_kit::SharedString>,
    color: u32,
) -> TextView {
    TextView::markdown(id, source).markdown_extensions(
        MarkdownExtensions::default()
            .plugin(MathPlugin {
                block: false,
                color,
            })
            .plugin(MathPlugin { block: true, color })
            .parser_revision(color as u64),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_math_delimiters_preserve_code_and_incomplete_streams() {
        let code = "```latex\n$$x^2$$\n\\[x\\]\n```\n`$$x$$`";
        assert_eq!(normalize(code), code);
        assert_eq!(
            normalize("公式：$$x^2$$结束"),
            "公式：\n\n$$\nx^2\n$$\n\n结束"
        );
        assert_eq!(normalize(r"\(x\)"), "$x$");
        assert_eq!(normalize("正在生成 $$\\frac{1}"), "正在生成 $$\\frac{1}");
        assert_eq!(normalize(r"价格 \$5 和 \$10"), r"价格 \$5 和 \$10");
    }
    #[test]
    fn fractions_chinese_scripts_and_units_render_in_both_themes() {
        for formula in [
            r"V_{\text{水2}}",
            r"V_{\text{锥剩}}=\frac{7}{8}V_{\text{锥总}}",
            r"30\times\left(\frac{1}{8}V_{\text{锥总}}\right)=\frac{7}{8}V_{\text{锥总}}+V_{\text{柱}}",
            r"\pi r^2 h_{\text{柱}}=\frac{23}{2}\pi r^2",
            r"h_{\text{柱}}=11.5\text{ 厘米}",
            r"\begin{aligned}x&=\sqrt{2}\\y&=\frac{1}{2}\end{aligned}",
        ] {
            for color in [0x253c32, 0xe4eae5] {
                let f = prepare(formula, true, color).expect(formula);
                assert!(f.width > 10. && f.height > 10. && f.baseline > 0.);
            }
        }
        assert!(prepare(r"\unknowncommand", true, 0).is_none());
    }
}
