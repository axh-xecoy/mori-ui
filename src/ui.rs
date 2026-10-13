use crate::{
    model::*,
    provider::{self, Event, Request},
    storage::{self, Store},
};
use anyhow::Context as _;

fn mori_logo() -> std::sync::Arc<gpui_kit::Image> {
    static LOGO: std::sync::OnceLock<std::sync::Arc<gpui_kit::Image>> = std::sync::OnceLock::new();
    LOGO.get_or_init(|| {
        std::sync::Arc::new(gpui_kit::Image::from_bytes(
            gpui_kit::ImageFormat::Png,
            include_bytes!("../assets/mori-128.png").to_vec(),
        ))
    })
    .clone()
}
use gpui_kit::component::{
    Disableable, Icon, Sizable, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    text::TextView,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
#[cfg(feature = "visual-test")]
mod capture;
mod config;
#[cfg(feature = "visual-test")]
pub use capture::start as capture;
mod views;

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Chat,
    Assistants,
    Tools,
    Settings,
}
enum Work {
    Models(String, anyhow::Result<Vec<String>>),
    Transcription(anyhow::Result<String>),
    Speech(anyhow::Result<std::path::PathBuf>),
    Image(String, anyhow::Result<Attachment>),
    Attach(String, anyhow::Result<Vec<Attachment>>),
    Imported(anyhow::Result<Library>),
    Notice(anyhow::Result<String>),
}
struct Generation {
    conversation: String,
    rx: mpsc::Receiver<Event>,
    cancel: Arc<AtomicBool>,
}
struct Approval {
    call: ToolCall,
    tx: mpsc::Sender<bool>,
}
#[derive(Clone, Copy)]
struct Palette {
    bg: u32,
    side: u32,
    card: u32,
    ink: u32,
    muted: u32,
    line: u32,
    accent: u32,
    soft: u32,
}
impl Palette {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                bg: 0x171d1c,
                side: 0x121716,
                card: 0x202826,
                ink: 0xe4eae5,
                muted: 0x94a49b,
                line: 0x303b37,
                accent: 0x87b99c,
                soft: 0x293d32,
            }
        } else {
            Self {
                bg: 0xfafbf8,
                side: 0xf1f3ed,
                card: 0xffffff,
                ink: 0x253c32,
                muted: 0x6c7b70,
                line: 0xe3e8de,
                accent: 0x356c50,
                soft: 0xe6eee3,
            }
        }
    }
}
pub struct Mori {
    library: Library,
    store: Option<Store>,
    window_handle: AnyWindowHandle,
    page: Page,
    composer: Entity<TextareaState>,
    search: Entity<InputState>,
    provider_name: Entity<InputState>,
    provider_url: Entity<InputState>,
    provider_key: Entity<InputState>,
    provider_models: Entity<TextareaState>,
    assistant_name: Entity<InputState>,
    assistant_prompt: Entity<TextareaState>,
    advanced: Entity<TextareaState>,
    title_input: Entity<InputState>,
    selected_provider: usize,
    selected_assistant: usize,
    model_picker: bool,
    rename: bool,
    sidebar: bool,
    image_mode: bool,
    attachments: Vec<Attachment>,
    generation: Option<Generation>,
    approval: Option<Approval>,
    expanded: HashSet<String>,
    toast: Option<String>,
    status: String,
    transcript: Option<String>,
    scroll: ScrollHandle,
    work_tx: mpsc::Sender<Work>,
    work_rx: mpsc::Receiver<Work>,
    media_busy: bool,
    draft_dirty: Option<std::time::Instant>,
    recording: Option<Arc<AtomicBool>>,
    last_deleted: Option<Conversation>,
    compact: bool,
    show_advanced: bool,
    temperature: Entity<InputState>,
    max_tokens: Entity<InputState>,
    image_model: Entity<InputState>,
    speech_model: Entity<InputState>,
    transcription_model: Entity<InputState>,
    mcp_name: Entity<InputState>,
    mcp_command: Entity<InputState>,
    mcp_args: Entity<TextareaState>,
    mcp_url: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}
impl Drop for Mori {
    fn drop(&mut self) {
        if let Some(g) = &self.generation {
            g.cancel.store(true, Ordering::Relaxed);
        }
        if let Some(stop) = &self.recording {
            stop.store(true, Ordering::Relaxed);
        }
        if let Some(store) = &self.store {
            let _ = store.save(&self.library);
        }
    }
}
fn icon(name: &str) -> Icon {
    static CATALOG: std::sync::OnceLock<
        std::collections::HashMap<String, gpui_kit::assets::IconName>,
    > = std::sync::OnceLock::new();
    let catalog = CATALOG.get_or_init(|| {
        gpui_kit::assets::IconName::ALL
            .iter()
            .map(|&icon| (icon.path().to_string(), icon))
            .collect()
    });
    let selected = catalog
        .get(&format!("icons/{name}.svg"))
        .copied()
        .unwrap_or(gpui_kit::assets::IconName::Info);
    Icon::default().path(selected.path()).size(px(18.))
}
fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, name: &str) -> Button {
    Button::new(id)
        .ghost()
        .h(px(32.))
        .label(label)
        .icon(icon(name))
}
fn tiny(id: impl Into<ElementId>, name: &str, tooltip: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .size(px(30.))
        .icon(icon(name))
        .tooltip(tooltip)
}
fn label(text: impl Into<SharedString>, color: u32) -> Div {
    div().text_xs().text_color(rgb(color)).child(text.into())
}
fn navigation_button(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    name: &str,
) -> Button {
    let text = text.into();
    Button::new(id)
        .ghost()
        .h(px(38.))
        .w_full()
        .accessibility_label(text.clone())
        .child(
            div()
                .flex()
                .items_center()
                .w_full()
                .gap_3()
                .child(icon(name))
                .child(div().text_sm().truncate().child(text)),
        )
}
fn heading(title: &str, description: &str, p: Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(p.muted))
                .child(description.to_string()),
        )
}
fn field(title: &str, input: impl IntoElement, p: Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(label(title.to_string(), p.muted))
        .child(input)
}
fn card(p: Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .p_5()
        .bg(rgb(p.card))
        .border_1()
        .border_color(rgb(p.line))
        .rounded_xl()
}

impl Mori {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut toast = None;
        let mut store = match Store::new() {
            Ok(s) => Some(s),
            Err(e) => {
                toast = Some(format!("本地存储不可用：{e:#}"));
                None
            }
        };
        let library = match store.as_ref().map(Store::load) {
            Some(Ok(lib)) => lib,
            Some(Err(e)) => {
                toast = Some(format!("{e:#}"));
                store = None;
                Library::default()
            }
            None => Library::default(),
        };
        Theme::change(
            if library.settings.dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            Some(window),
            cx,
        );
        Self::tint(cx, library.settings.dark);
        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("有什么想聊的？也可以附上图片或文档…")
                .auto_grow(3, 9)
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索对话"));
        let provider_name = cx.new(|cx| InputState::new(window, cx).placeholder("服务商名称"));
        let provider_url =
            cx.new(|cx| InputState::new(window, cx).placeholder("https://api.example.com/v1"));
        let provider_key = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("新的 API Key（留空保留现有密钥）")
                .masked(true)
        });
        let provider_models = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("每行一个模型 ID")
                .auto_grow(3, 6)
        });
        let assistant_name = cx.new(|cx| InputState::new(window, cx));
        let assistant_prompt = cx.new(|cx| TextareaState::new(window, cx).auto_grow(7, 12));
        let advanced = cx.new(|cx| TextareaState::new(window, cx).auto_grow(15, 25));
        let title_input = cx.new(|cx| InputState::new(window, cx));
        let temperature = cx.new(|cx| {
            InputState::new(window, cx).default_value(library.settings.temperature.to_string())
        });
        let max_tokens = cx.new(|cx| {
            InputState::new(window, cx).default_value(library.settings.max_tokens.to_string())
        });
        let image_model = cx.new(|cx| {
            InputState::new(window, cx).default_value(library.settings.image_model.clone())
        });
        let speech_model = cx.new(|cx| {
            InputState::new(window, cx).default_value(library.settings.speech_model.clone())
        });
        let transcription_model = cx.new(|cx| {
            InputState::new(window, cx).default_value(library.settings.transcription_model.clone())
        });
        let mcp_name = cx.new(|cx| InputState::new(window, cx).placeholder("例如：我的资料库"));
        let mcp_command =
            cx.new(|cx| InputState::new(window, cx).placeholder("例如：npx.cmd 或程序的完整路径"));
        let mcp_args = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("每行一个启动参数")
                .auto_grow(2, 5)
        });
        let mcp_url = cx.new(|cx| {
            InputState::new(window, cx).placeholder("https://example.com/mcp（本地程序留空）")
        });
        let (work_tx, work_rx) = mpsc::channel();
        let subscriptions = vec![
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.observe(&composer, |this, _, cx| {
                this.stash_draft(cx);
                this.draft_dirty = Some(std::time::Instant::now());
            }),
            cx.subscribe_in(&composer, window, |this, _, event, window, cx| {
                if matches!(
                    event,
                    InputEvent::PressEnter {
                        secondary: true,
                        shift: false
                    }
                ) {
                    this.send(window, cx);
                }
            }),
            cx.subscribe_in(&title_input, window, |this, _, event, window, cx| {
                if this.rename && matches!(event, InputEvent::PressEnter { .. }) {
                    this.commit_title(window, cx);
                }
            }),
        ];
        let mut this = Self {
            library,
            store,
            window_handle: window.window_handle(),
            page: Page::Chat,
            composer,
            search,
            provider_name,
            provider_url,
            provider_key,
            provider_models,
            assistant_name,
            assistant_prompt,
            advanced,
            title_input,
            selected_provider: 0,
            selected_assistant: 0,
            model_picker: false,
            rename: false,
            sidebar: true,
            image_mode: false,
            attachments: vec![],
            generation: None,
            approval: None,
            expanded: HashSet::new(),
            toast,
            status: String::new(),
            transcript: None,
            scroll: ScrollHandle::new(),
            work_tx,
            work_rx,
            media_busy: false,
            draft_dirty: None,
            recording: None,
            last_deleted: None,
            compact: false,
            show_advanced: false,
            temperature,
            max_tokens,
            image_model,
            speech_model,
            transcription_model,
            mcp_name,
            mcp_command,
            mcp_args,
            mcp_url,
            _subscriptions: subscriptions,
        };
        if this.library.providers.is_empty() {
            this.library.providers = Provider::presets();
        }
        if this.library.assistants.is_empty() {
            this.library.assistants = Assistant::presets();
        }
        if let Some(c) = this.current() {
            let draft = c.draft.clone();
            this.attachments = c.draft_attachments.clone();
            this.composer
                .update(cx, |s, cx| s.set_value(draft, window, cx));
        }
        this.load_provider(0, window, cx);
        this.load_assistant(0, window, cx);
        this.load_advanced(window, cx);
        cx.spawn(async move |entity, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                if entity.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        this
    }
    fn tint(cx: &mut App, dark: bool) {
        let p = Palette::new(dark);
        Theme::update(cx, |theme| {
            let foreground = rgb(if dark { 0x17281e } else { 0xffffff }).into();
            let hover = rgb(if dark { 0x9cc9ae } else { 0x2b5d44 }).into();
            let active = rgb(if dark { 0x75a98c } else { 0x234e39 }).into();
            theme.primary = rgb(p.accent).into();
            theme.primary_foreground = foreground;
            theme.primary_hover = hover;
            theme.primary_active = active;
            theme.button_primary = rgb(p.accent).into();
            theme.button_primary_foreground = foreground;
            theme.button_primary_hover = hover;
            theme.button_primary_active = active;
            theme.background = rgb(p.bg).into();
            theme.foreground = rgb(p.ink).into();
            theme.border = rgb(p.line).into();
            theme.ring = rgb(p.accent).into();
            theme.accent = rgb(p.soft).into();
            theme.accent_foreground = rgb(p.accent).into();
            theme.muted = rgb(p.soft).into();
            theme.muted_foreground = rgb(p.muted).into();
            theme.secondary = rgb(p.soft).into();
            theme.secondary_foreground = rgb(p.ink).into();
            theme.radius = px(8.);
            theme.font_family = "Microsoft YaHei".into();
        });
    }
    fn current(&self) -> Option<&Conversation> {
        self.library
            .active_id
            .as_ref()
            .and_then(|id| self.library.conversations.iter().find(|c| &c.id == id))
    }
    fn current_mut(&mut self) -> Option<&mut Conversation> {
        let id = self.library.active_id.clone()?;
        self.library.conversations.iter_mut().find(|c| c.id == id)
    }
    fn persist(&mut self) {
        if let Some(store) = &self.store
            && let Err(e) = store.save(&self.library)
        {
            self.toast = Some(format!("保存失败：{e:#}"));
        }
    }
    fn stash_draft(&mut self, cx: &App) {
        let text = self.composer.read(cx).value().to_string();
        let attachments = self.attachments.clone();
        if let Some(c) = self.current_mut() {
            c.draft = text;
            c.draft_attachments = attachments;
        }
    }
    fn new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stash_draft(cx);
        let (provider, model, assistant) = self
            .current()
            .map(|c| {
                (
                    c.provider_id.clone(),
                    c.model.clone(),
                    c.assistant_id.clone(),
                )
            })
            .unwrap_or_else(|| {
                let p = &self.library.providers[self.selected_provider];
                (
                    p.id.clone(),
                    p.models.first().cloned().unwrap_or_default(),
                    "general".into(),
                )
            });
        let c = Conversation::new(provider, model, assistant);
        self.library.active_id = Some(c.id.clone());
        self.library.conversations.insert(0, c);
        self.composer
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.attachments.clear();
        self.page = Page::Chat;
        self.model_picker = false;
        self.rename = false;
        self.persist();
        cx.notify();
    }
    fn select_chat(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.stash_draft(cx);
        self.library.active_id = Some(id);
        let draft = self.current().map(|c| c.draft.clone()).unwrap_or_default();
        self.composer
            .update(cx, |s, cx| s.set_value(draft, window, cx));
        self.attachments = self
            .current()
            .map(|c| c.draft_attachments.clone())
            .unwrap_or_default();
        self.page = Page::Chat;
        self.rename = false;
        self.persist();
        self.scroll.scroll_to_bottom();
        cx.notify();
    }
    fn rename_conversation(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.select_chat(id, window, cx);
        let title = self.current().map(|c| c.title.clone()).unwrap_or_default();
        self.title_input.update(cx, |s, cx| {
            s.set_value(title, window, cx);
            s.focus(window, cx);
        });
        self.rename = true;
        cx.notify();
    }
    fn commit_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.title_input.read(cx).value().trim().to_string();
        if !text.is_empty()
            && let Some(c) = self.current_mut()
        {
            c.title = text;
        }
        self.rename = false;
        self.persist();
        self.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }
    fn delete_conversation(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .generation
            .as_ref()
            .is_some_and(|g| g.conversation == id)
        {
            self.toast = Some("先停止回复，再删除这条会话".into());
            cx.notify();
            return;
        }
        self.stash_draft(cx);
        if let Some(index) = self.library.conversations.iter().position(|c| c.id == id) {
            self.last_deleted = Some(self.library.conversations.remove(index));
            if self.library.active_id.as_deref() == Some(id) {
                self.library.active_id = self.library.conversations.first().map(|c| c.id.clone());
                let draft = self.current().map(|c| c.draft.clone()).unwrap_or_default();
                self.attachments = self
                    .current()
                    .map(|c| c.draft_attachments.clone())
                    .unwrap_or_default();
                self.composer
                    .update(cx, |s, cx| s.set_value(draft, window, cx));
                self.rename = false;
            }
            self.toast = Some("会话已删除。可以撤销这次操作。".into());
            self.persist();
            cx.notify();
        }
    }
    fn undo_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(c) = self.last_deleted.take() {
            let id = c.id.clone();
            self.library.conversations.insert(0, c);
            self.select_chat(id, window, cx);
            self.toast = Some("会话已恢复".into());
        }
        cx.notify();
    }
    fn navigate(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        self.stash_draft(cx);
        self.page = page;
        self.model_picker = false;
        if page == Page::Tools {
            self.load_advanced(window, cx);
        }
        self.persist();
        cx.notify();
    }
    fn load_provider(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_provider = index;
        let p = &self.library.providers[index];
        let name = p.name.clone();
        let url = p.base_url.clone();
        let models = p.models.join("\n");
        self.provider_name
            .update(cx, |s, cx| s.set_value(name, window, cx));
        self.provider_url
            .update(cx, |s, cx| s.set_value(url, window, cx));
        self.provider_key
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.provider_models
            .update(cx, |s, cx| s.set_value(models, window, cx));
        cx.notify();
    }
    fn save_provider(&mut self, cx: &mut Context<Self>) -> bool {
        let name = self.provider_name.read(cx).value().to_string();
        let url = self.provider_url.read(cx).value().to_string();
        let key = self.provider_key.read(cx).value().to_string();
        if name.trim().is_empty() || !(url.starts_with("https://") || url.starts_with("http://")) {
            self.toast = Some("请填写名称和有效的 HTTP / HTTPS 地址".into());
            cx.notify();
            return false;
        }
        let p = &mut self.library.providers[self.selected_provider];
        if !key.is_empty()
            && let Err(e) = storage::set_api_key(p, &key)
        {
            self.toast = Some(format!("系统凭据库写入失败：{e:#}"));
            cx.notify();
            return false;
        }
        p.name = name.trim().into();
        p.base_url = url.trim().trim_end_matches('/').into();
        p.models = self
            .provider_models
            .read(cx)
            .value()
            .lines()
            .flat_map(|s| s.split(','))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        self.persist();
        self.toast = Some("服务商设置已保存".into());
        cx.notify();
        true
    }
    fn load_assistant(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_assistant = index;
        let a = &self.library.assistants[index];
        let name = a.name.clone();
        let prompt = a.prompt.clone();
        self.assistant_name
            .update(cx, |s, cx| s.set_value(name, window, cx));
        self.assistant_prompt
            .update(cx, |s, cx| s.set_value(prompt, window, cx));
        cx.notify();
    }
    fn load_advanced(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let json = serde_json::to_string_pretty(&self.library.settings).unwrap_or_default();
        self.advanced
            .update(cx, |s, cx| s.set_value(json, window, cx));
    }
    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(g) = self.generation.take() {
            g.cancel.store(true, Ordering::Relaxed);
            if let Some(c) = self
                .library
                .conversations
                .iter_mut()
                .find(|c| c.id == g.conversation)
            {
                for m in &mut c.messages {
                    for t in &mut m.tools {
                        if t.result.is_none() {
                            t.result = Some("用户中断了工具请求。".into());
                        }
                    }
                }
            }
        }
        self.approval = None;
        self.status = "已停止，已收到的内容已保存".into();
        self.persist();
        cx.notify();
    }
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.generation.is_some() || self.media_busy {
            return;
        }
        let text = self.composer.read(cx).value().to_string();
        if text.trim().is_empty() && self.attachments.is_empty() {
            return;
        }
        if self.current().is_none() {
            let attachments = std::mem::take(&mut self.attachments);
            self.new_chat(window, cx);
            self.attachments = attachments;
            self.composer
                .update(cx, |s, cx| s.set_value(text.clone(), window, cx));
        }
        let current = self.current().unwrap();
        let Some(p) = self
            .library
            .providers
            .iter()
            .find(|p| p.id == current.provider_id && p.enabled)
            .cloned()
        else {
            self.toast = Some("请先选择一个已启用的服务商".into());
            cx.notify();
            return;
        };
        if current.model.is_empty() && !self.image_mode {
            self.model_picker = true;
            self.toast = Some("先选择模型；本地服务可在设置中获取模型列表".into());
            cx.notify();
            return;
        }
        let mut message = Message::new("user", text.trim());
        message.attachments = std::mem::take(&mut self.attachments);
        let c = self.current_mut().unwrap();
        if c.messages.is_empty() {
            c.title = text.trim().chars().take(24).collect();
            if c.title.is_empty() {
                c.title = "聊聊这份资料".into();
            }
        }
        c.messages.push(message);
        c.draft.clear();
        c.draft_attachments.clear();
        c.updated = chrono::Local::now();
        self.composer
            .update(cx, |s, cx| s.set_value("", window, cx));
        if self.image_mode {
            let id = self.current().unwrap().id.clone();
            let model = self.library.settings.image_model.clone();
            let tx = self.work_tx.clone();
            self.media_busy = true;
            self.status = "正在生成图片…".into();
            std::thread::spawn(move || {
                let result =
                    Store::new().and_then(|s| provider::generate_image(&p, &text, &model, &s));
                let _ = tx.send(Work::Image(id, result));
            });
        } else {
            self.start_generation(p, cx);
        }
        self.persist();
        self.scroll.scroll_to_bottom();
        cx.notify();
    }
    fn start_generation(&mut self, p: Provider, cx: &mut Context<Self>) {
        let c = self.current().unwrap().clone();
        let prompt = self
            .library
            .assistants
            .iter()
            .find(|a| a.id == c.assistant_id)
            .map(|a| a.prompt.clone())
            .unwrap_or_default();
        let request = Request {
            provider: p,
            conversation: c.clone(),
            prompt,
            settings: self.library.settings.clone(),
        };
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        self.generation = Some(Generation {
            conversation: c.id,
            rx,
            cancel,
        });
        self.status = "正在连接模型…".into();
        self.approval = None;
        std::thread::spawn(move || provider::run(request, tx, worker_cancel));
        cx.notify();
    }
    fn retry(&mut self, cx: &mut Context<Self>) {
        if self.generation.is_some() || self.media_busy {
            return;
        }
        let Some(c) = self.current_mut() else {
            return;
        };
        let Some(index) = c.messages.iter().rposition(|m| m.role == "user") else {
            return;
        };
        c.messages.truncate(index + 1);
        let provider_id = c.provider_id.clone();
        if let Some(p) = self
            .library
            .providers
            .iter()
            .find(|p| p.id == provider_id)
            .cloned()
        {
            self.start_generation(p, cx);
            self.persist();
        }
    }
    fn branch(&mut self, index: usize, edit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(original) = self.current() else {
            return;
        };
        let mut branch = original.clone();
        branch.id = id();
        branch.title = format!("{} · 分支", branch.title);
        branch.pinned = false;
        branch.updated = chrono::Local::now();
        let edited = branch.messages[index].clone();
        branch
            .messages
            .truncate(if edit { index } else { index + 1 });
        branch.draft = if edit {
            edited.content.clone()
        } else {
            String::new()
        };
        branch.draft_attachments = if edit {
            edited.attachments.clone()
        } else {
            vec![]
        };
        let new_id = branch.id.clone();
        self.library.conversations.insert(0, branch);
        self.select_chat(new_id, window, cx);
        if edit {
            self.attachments = edited.attachments;
        }
        self.toast = Some("已创建会话分支，原对话保留".into());
        cx.notify();
    }
    fn poll(&mut self, cx: &mut Context<Self>) {
        let follow = (self.scroll.offset().y + self.scroll.max_offset().y).abs() < px(100.);
        if self
            .draft_dirty
            .is_some_and(|t| t.elapsed() > Duration::from_millis(800))
        {
            self.draft_dirty = None;
            self.stash_draft(cx);
            self.persist();
        }
        let mut changed = false;
        let mut done = false;
        let events: Vec<_> = self
            .generation
            .as_ref()
            .map(|g| g.rx.try_iter().take(256).collect())
            .unwrap_or_default();
        let cid = self.generation.as_ref().map(|g| g.conversation.clone());
        for event in events {
            changed = true;
            match event {
                Event::Message(message) => {
                    if let Some(c) = self
                        .library
                        .conversations
                        .iter_mut()
                        .find(|c| Some(&c.id) == cid.as_ref())
                    {
                        if let Some(m) = c.messages.iter_mut().find(|m| m.id == message.id) {
                            *m = message;
                        } else {
                            c.messages.push(message);
                        }
                        c.updated = chrono::Local::now();
                    }
                    self.status = "正在回复…".into();
                }
                Event::Approval(call, tx) => {
                    self.approval = Some(Approval { call, tx });
                    self.status = "等待你的工具执行确认".into();
                }
                Event::Status(s) => self.status = s,
                Event::Error(e) => {
                    if let Some(c) = self
                        .library
                        .conversations
                        .iter_mut()
                        .find(|c| Some(&c.id) == cid.as_ref())
                    {
                        if let Some(m) = c.messages.last_mut().filter(|m| m.role == "assistant") {
                            m.error = Some(e.clone());
                        } else {
                            let mut m = Message::new("assistant", "");
                            m.error = Some(e.clone());
                            c.messages.push(m);
                        }
                    }
                    self.toast = Some(e);
                }
                Event::Done => {
                    done = true;
                    self.status.clear();
                }
            }
        }
        if done {
            self.generation = None;
            self.approval = None;
            self.persist();
        }
        let works: Vec<_> = self.work_rx.try_iter().collect();
        for work in works {
            changed = true;
            match work {
                Work::Models(id, result) => match result {
                    Ok(models) => {
                        let text = models.join("\n");
                        if let Some(p) = self.library.providers.iter_mut().find(|p| p.id == id) {
                            p.models = models;
                        }
                        if self.library.providers[self.selected_provider].id == id {
                            let input = self.provider_models.clone();
                            let _ = self.window_handle.update(cx, |_, window, cx| {
                                input.update(cx, |s, cx| s.set_value(text, window, cx))
                            });
                        }
                        self.persist();
                        self.toast = Some("连接成功，模型列表已更新".into());
                    }
                    Err(e) => self.toast = Some(format!("获取模型失败：{e:#}")),
                },
                Work::Transcription(result) => {
                    self.recording = None;
                    self.media_busy = false;
                    self.status.clear();
                    match result {
                        Ok(text) => {
                            self.transcript = Some(text);
                            self.toast = Some("转写完成，可以插入输入框".into());
                        }
                        Err(e) => self.toast = Some(format!("转写失败：{e:#}")),
                    }
                }
                Work::Speech(result) => {
                    self.media_busy = false;
                    self.status.clear();
                    match result {
                        Ok(path) => {
                            cx.open_url(&format!(
                                "file:///{}",
                                path.to_string_lossy().replace('\\', "/")
                            ));
                        }
                        Err(e) => self.toast = Some(format!("朗读失败：{e:#}")),
                    }
                }
                Work::Image(id, result) => {
                    self.media_busy = false;
                    self.status.clear();
                    if let Some(c) = self.library.conversations.iter_mut().find(|c| c.id == id) {
                        match result {
                            Ok(a) => {
                                let mut m = Message::new("assistant", "为你生成的图片");
                                m.attachments.push(a);
                                c.messages.push(m);
                            }
                            Err(e) => {
                                let mut m = Message::new("assistant", "");
                                m.error = Some(format!("{e:#}"));
                                c.messages.push(m);
                            }
                        }
                    }
                    self.persist();
                }
                Work::Attach(id, result) => match result {
                    Ok(a) => {
                        if self.library.active_id.as_deref() == Some(&id) {
                            self.attachments.extend(a);
                            self.stash_draft(cx);
                        } else if let Some(c) =
                            self.library.conversations.iter_mut().find(|c| c.id == id)
                        {
                            c.draft_attachments.extend(a);
                            self.toast = Some("附件已添加到原会话草稿".into());
                        } else {
                            self.toast = Some("原会话已删除，附件未加入其他会话".into());
                        }
                        self.persist();
                    }
                    Err(e) => self.toast = Some(format!("添加附件失败：{e:#}")),
                },
                Work::Imported(result) => match result {
                    Ok(lib) => {
                        for c in lib.conversations {
                            if !self
                                .library
                                .conversations
                                .iter()
                                .any(|existing| existing.id == c.id)
                            {
                                self.library.conversations.push(c);
                            }
                        }
                        self.persist();
                        self.toast = Some("会话已导入；服务商和密钥沿用本机设置".into());
                    }
                    Err(e) => self.toast = Some(format!("导入失败：{e:#}")),
                },
                Work::Notice(result) => {
                    self.toast = Some(match result {
                        Ok(s) => s,
                        Err(e) => format!("{e:#}"),
                    })
                }
            }
        }
        if changed {
            if follow && cid == self.library.active_id && self.page == Page::Chat {
                self.scroll.scroll_to_bottom();
            }
            cx.notify();
        }
    }
    fn attachment_target(&mut self, window: &mut Window, cx: &mut Context<Self>) -> String {
        if self.current().is_none() {
            let draft = self.composer.read(cx).value().to_string();
            let attachments = self.attachments.clone();
            self.new_chat(window, cx);
            self.composer
                .update(cx, |s, cx| s.set_value(draft, window, cx));
            self.attachments = attachments;
            self.stash_draft(cx);
        }
        self.library
            .active_id
            .clone()
            .expect("attachment target exists")
    }
    fn attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.attachment_target(window, cx);
        let tx = self.work_tx.clone();
        std::thread::spawn(move || {
            if let Some(paths) = rfd::FileDialog::new()
                .set_title("添加图片或文档")
                .add_filter(
                    "图片与文档",
                    &[
                        "png", "jpg", "jpeg", "webp", "gif", "pdf", "txt", "md", "csv", "json",
                        "rs", "py", "html", "xml",
                    ],
                )
                .pick_files()
            {
                let result = Store::new().and_then(|s| paths.iter().map(|p| s.attach(p)).collect());
                let _ = tx.send(Work::Attach(id, result));
            }
        });
    }
    fn attach_paths(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = self.attachment_target(window, cx);
        let tx = self.work_tx.clone();
        std::thread::spawn(move || {
            let result =
                Store::new().and_then(|store| paths.iter().map(|p| store.attach(p)).collect());
            let _ = tx.send(Work::Attach(id, result));
        });
    }
    fn microphone(&mut self, cx: &mut Context<Self>) {
        if let Some(stop) = self.recording.take() {
            stop.store(true, Ordering::Relaxed);
            self.status = "正在转写录音…".into();
            cx.notify();
            return;
        }
        if self.media_busy || self.generation.is_some() {
            return;
        }
        let Some(p) = self.selected_chat_provider() else {
            return;
        };
        if p.protocol != Protocol::OpenAi {
            self.toast = Some("语音输入需要选择支持音频转写的 OpenAI 兼容服务商".into());
            cx.notify();
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.recording = Some(stop.clone());
        self.media_busy = true;
        self.status = "正在录音 · 点击麦克风结束（最多 2 分钟）".into();
        let model = self.library.settings.transcription_model.clone();
        let tx = self.work_tx.clone();
        std::thread::spawn(move || {
            let result = Store::new()
                .and_then(|s| {
                    crate::audio::record(
                        &stop,
                        s.root.join("attachments").join(format!("{}.wav", id())),
                    )
                })
                .and_then(|path| provider::transcribe(&p, &path, &model));
            let _ = tx.send(Work::Transcription(result));
        });
        cx.notify();
    }
    fn export(&mut self, all: bool) {
        let tx = self.work_tx.clone();
        let library = self.library.clone();
        let current = self.current().cloned();
        std::thread::spawn(move || {
            let dialog = rfd::FileDialog::new().set_file_name(if all {
                "mori-conversations.json"
            } else {
                "conversation.md"
            });
            if let Some(path) = dialog.save_file() {
                let result = (|| -> anyhow::Result<String> {
                    let text = if all {
                        serde_json::to_string_pretty(&library)?
                    } else {
                        current.context("没有可导出的会话")?.markdown()
                    };
                    std::fs::write(path, text)?;
                    Ok("导出完成".into())
                })();
                let _ = tx.send(Work::Notice(result));
            }
        });
    }
    fn import(&mut self) {
        let tx = self.work_tx.clone();
        std::thread::spawn(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Mori 会话库", &["json"])
                .pick_file()
            {
                let result = (|| {
                    let bytes = std::fs::read(path)?;
                    let lib: Library = serde_json::from_slice(&bytes)?;
                    if lib.conversations.iter().any(|c| {
                        c.messages
                            .iter()
                            .any(|m| !matches!(m.role.as_str(), "user" | "assistant"))
                    }) {
                        anyhow::bail!("导入数据包含无效消息角色");
                    }
                    Ok(lib)
                })();
                let _ = tx.send(Work::Imported(result));
            }
        });
    }
    fn transcribe(&mut self, cx: &mut Context<Self>) {
        if self.media_busy {
            return;
        }
        let Some(p) = self.selected_chat_provider() else {
            return;
        };
        let model = self.library.settings.transcription_model.clone();
        let tx = self.work_tx.clone();
        self.media_busy = true;
        self.status = "选择音频并转写…".into();
        std::thread::spawn(move || {
            let result = if let Some(path) = rfd::FileDialog::new()
                .add_filter("音频", &["mp3", "wav", "m4a", "ogg", "webm", "flac"])
                .pick_file()
            {
                provider::transcribe(&p, &path, &model)
            } else {
                Err(anyhow::anyhow!("未选择音频"))
            };
            let _ = tx.send(Work::Transcription(result));
        });
        cx.notify();
    }
    fn selected_chat_provider(&self) -> Option<Provider> {
        let id = self
            .current()
            .map(|c| c.provider_id.as_str())
            .unwrap_or(&self.library.providers[self.selected_provider].id);
        self.library.providers.iter().find(|p| p.id == id).cloned()
    }
    fn speak(&mut self, text: String, cx: &mut Context<Self>) {
        if self.media_busy {
            return;
        }
        let Some(p) = self.selected_chat_provider() else {
            return;
        };
        let model = self.library.settings.speech_model.clone();
        let tx = self.work_tx.clone();
        self.media_busy = true;
        self.status = "正在准备朗读…".into();
        std::thread::spawn(move || {
            let result = Store::new().and_then(|s| provider::speak(&p, &text, &model, &s));
            let _ = tx.send(Work::Speech(result));
        });
        cx.notify();
    }
}
