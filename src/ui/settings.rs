use super::*;

fn setting_row(title: &str, description: &str, control: impl IntoElement, p: Palette) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .py_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .min_w_0()
                .flex_1()
                .child(div().font_weight(FontWeight::MEDIUM).child(tr(title)))
                .child(label(description, p.muted)),
        )
        .child(control)
}

impl Mori {
    pub(super) fn edit_model(&mut self, model: &str, window: &mut Window, cx: &mut Context<Self>) {
        let config = self.library.providers[self.selected_provider]
            .model_configs
            .get(model)
            .cloned()
            .unwrap_or_default();
        let mut parameters = config.parameters;
        self.model_id_input
            .update(cx, |s, cx| s.set_value(model, window, cx));
        self.model_name_input
            .update(cx, |s, cx| s.set_value(config.name, window, cx));
        for (key, input) in [
            ("temperature", &self.model_temperature),
            ("top_p", &self.model_top_p),
            ("max_tokens", &self.model_max_tokens),
        ] {
            let value = parameters
                .remove(key)
                .filter(|v| !v.is_null())
                .map(|v| v.to_string())
                .unwrap_or_default();
            input.update(cx, |s, cx| s.set_value(value, window, cx));
        }
        self.model_parameters.update(cx, |s, cx| {
            s.set_value(
                serde_json::to_string_pretty(&parameters).unwrap(),
                window,
                cx,
            )
        });
        self.editing_model = model.into();
        self.model_editor = true;
        cx.notify();
    }
    pub(super) fn save_model(&mut self, cx: &mut Context<Self>) {
        let result = (|| -> anyhow::Result<(String, ModelConfig)> {
            let model = self.model_id_input.read(cx).value().trim().to_string();
            anyhow::ensure!(!model.is_empty(), "模型 ID 不能为空");
            let value: serde_json::Value =
                serde_json::from_str(&self.model_parameters.read(cx).value())?;
            let mut parameters = value
                .as_object()
                .cloned()
                .context("自定义参数必须是 JSON 对象")?;
            for (key, input) in [
                ("temperature", &self.model_temperature),
                ("top_p", &self.model_top_p),
                ("max_tokens", &self.model_max_tokens),
            ] {
                let text = input.read(cx).value();
                let text = text.trim();
                if text.is_empty() {
                    let native_key = if key == "max_tokens" {
                        "num_predict"
                    } else {
                        key
                    };
                    let native = parameters
                        .get("options")
                        .and_then(|v| v.as_object())
                        .is_some_and(|v| v.contains_key(native_key));
                    if !native
                        && !(key == "max_tokens"
                            && parameters.contains_key("max_completion_tokens"))
                    {
                        parameters.entry(key).or_insert(serde_json::Value::Null);
                    }
                } else {
                    let value: serde_json::Value = serde_json::from_str(text)?;
                    if key == "max_tokens" {
                        anyhow::ensure!(
                            value.as_u64().is_some_and(|v| v > 0),
                            "输出上限必须是正整数，留空不设置上限"
                        );
                    }
                    parameters.insert(key.into(), value);
                }
            }
            provider::validate_parameters(&parameters)?;
            Ok((
                model,
                ModelConfig {
                    name: self.model_name_input.read(cx).value().trim().into(),
                    parameters,
                },
            ))
        })();
        match result {
            Ok((model, config)) => {
                let p = &mut self.library.providers[self.selected_provider];
                if !p.models.contains(&model) {
                    p.models.push(model.clone());
                }
                p.model_configs.insert(model, config);
                self.model_editor = false;
                self.persist();
                self.show_toast("模型设置已保存".into());
            }
            Err(e) => self.show_toast(format!("模型设置无效：{e:#}")),
        }
        cx.notify();
    }
    fn render_model_settings(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let provider = &self.library.providers[self.selected_provider];
        div().flex().flex_col().gap_3()
            .child(div().flex().flex_wrap().gap_2().items_center().justify_between()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(tr("模型与高级参数")))
                .when(!self.model_editor,|d|d.child(button("add-model","添加模型","plus").on_click(cx.listener(|this,_,w,cx|this.edit_model("",w,cx)))))
                .when(self.model_editor,|d|d.child(div().flex().items_center().gap_2()
                    .child(button("cancel-model-config","取消","x").on_click(cx.listener(|this,_,_,cx|{this.model_editor=false;cx.notify();})))
                    .child(Button::new("save-model-top").primary().label(tr("保存模型设置")).on_click(cx.listener(|this,_,_,cx|this.save_model(cx)))))))
            .when(provider.models.is_empty(),|d|d.child(label("连接获取模型，或手动添加模型。",p.muted)))
            .when(!self.model_editor,|d|d.child(div().id("provider-model-list").flex().flex_col().gap_2().max_h(px(240.)).overflow_y_scroll().children(provider.models.iter().map(|model|{
                let edit=model.clone();let remove=model.clone();let shown=provider.model_name(model);
                div().flex().items_center().gap_2().p_3().rounded_lg().bg(rgb(p.bg))
                    .child(div().flex_1().min_w_0().child(div().child(shown.clone()))
                        .when(shown!=*model,|d|d.child(label(model.clone(),p.muted))))
                    .child(tiny(format!("edit-model-{model}"),"sliders-horizontal","模型高级参数")
                        .on_click(cx.listener(move|this,_,w,cx|this.edit_model(&edit,w,cx))))
                    .child(tiny(format!("remove-model-{model}"),"trash","移除模型")
                        .on_click(cx.listener(move|this,_,_,cx|{this.library.providers[this.selected_provider].models.retain(|m|m!=&remove);this.model_editor=false;this.persist();cx.notify();})))
            }))))
            .when(self.model_editor,|d|d.child(div().flex().flex_col().gap_4().p_4().border_1().border_color(rgb(p.line)).rounded_lg()
                .child(field("模型 ID",Input::new(&self.model_id_input).disabled(!self.editing_model.is_empty()),p))
                .child(field("显示名称（留空使用模型 ID）",Input::new(&self.model_name_input),p))
                .child(div().flex().flex_wrap().gap_3()
                    .child(field("Temperature",Input::new(&self.model_temperature),p).flex_1().min_w(px(120.)))
                    .child(field("Top P",Input::new(&self.model_top_p),p).flex_1().min_w(px(120.)))
                    .child(field("输出上限（tokens）",Input::new(&self.model_max_tokens),p).flex_1().min_w(px(150.))))
                .child(label("采样参数留空使用服务商默认；输出留空不设置客户端上限。Claude 必须填写输出上限。",p.muted))
                .child(field("更多参数（JSON）",Textarea::new(&self.model_parameters).font_family("Cascadia Code"),p))
                .child(label("支持 top_k、seed、stop、惩罚、reasoning_effort，以及 options 等服务商原生嵌套参数。null 表示不发送该参数。",p.muted))))
    }
    pub(super) fn set_language(
        &mut self,
        language: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.library.settings.language = language.into();
        crate::i18n::set_language(language);
        window.set_window_title(&format!("Mori · {}", tr("留一点空间给灵感")));
        for (input, text) in [
            (&self.search, "搜索对话"),
            (&self.provider_name, "连接名称"),
            (&self.provider_key, "新的 API Key（留空保留现有密钥）"),
            (&self.model_name_input, "默认使用模型 ID"),
            (&self.model_temperature, "服务商默认"),
            (&self.model_top_p, "服务商默认"),
            (&self.model_max_tokens, "不设置客户端上限"),
            (&self.mcp_name, "例如：我的资料库"),
            (&self.mcp_command, "例如：npx.cmd 或程序的完整路径"),
            (&self.mcp_url, "https://example.com/mcp（本地程序留空）"),
        ] {
            input.update(cx, |s, cx| s.set_placeholder(tr(text), window, cx));
        }
        for (input, text) in [
            (&self.composer, "有什么想聊的？也可以附上图片或文档…"),
            (&self.mcp_args, "每行一个启动参数"),
        ] {
            input.update(cx, |s, cx| s.set_placeholder(tr(text), window, cx));
        }
        self.persist();
        cx.notify();
    }
    pub(super) fn render_settings(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("settings-page")
            .size_full()
            .max_w(px(1040.))
            .mx_auto()
            .overflow_y_scroll()
            .p_8()
            .flex()
            .flex_col()
            .gap_6()
            .child(heading(
                "让 Mori 更合你的习惯。",
                "外观、语言、输入方式与模型连接，都在这里。",
                p,
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .p_1()
                    .rounded_lg()
                    .bg(rgb(p.soft))
                    .children(
                        [
                            (SettingsTab::General, "通用", "sliders-horizontal"),
                            (SettingsTab::Connections, "模型连接", "network"),
                            (SettingsTab::Data, "数据与隐私", "folder-lock"),
                        ]
                        .into_iter()
                        .map(|(tab, title, name)| {
                            button(format!("settings-tab-{title}"), title, name)
                                .h(px(40.))
                                .when(self.settings_tab == tab, |b| {
                                    b.bg(rgb(p.card)).text_color(rgb(p.accent))
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.settings_tab = tab;
                                    this.provider_picker = false;
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .child(match self.settings_tab {
                SettingsTab::General => self.render_general(p, cx).into_any_element(),
                SettingsTab::Connections => self.render_connections(p, cx).into_any_element(),
                SettingsTab::Data => self.render_data(p, cx).into_any_element(),
            })
    }
    fn render_general(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let english = self.library.settings.language == "en";
        card(p)
            .p_6()
            .gap_2()
            .child(setting_row(
                "界面语言",
                "选择你熟悉的语言，聊天内容保持原样。",
                div().flex().gap_2().children(
                    [("zh-CN", "简体中文"), ("en", "English")].into_iter().map(
                        |(language, name)| {
                            button(format!("language-{language}"), name, "languages")
                                .when((language == "en") == english, |b| {
                                    b.bg(rgb(p.soft)).text_color(rgb(p.accent))
                                })
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    this.set_language(language, w, cx)
                                }))
                        },
                    ),
                ),
                p,
            ))
            .child(div().h(px(1.)).bg(rgb(p.line)))
            .child(setting_row(
                "外观",
                "柔和的颜色，适合白天与夜晚。",
                div().flex().gap_2().children(
                    [(false, "浅色", "sun"), (true, "深色", "moon")]
                        .into_iter()
                        .map(|(dark, title, name)| {
                            button(format!("appearance-{dark}"), title, name)
                                .when(self.library.settings.dark == dark, |b| {
                                    b.bg(rgb(p.soft)).text_color(rgb(p.accent))
                                })
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    this.library.settings.dark = dark;
                                    Theme::change(
                                        if dark {
                                            ThemeMode::Dark
                                        } else {
                                            ThemeMode::Light
                                        },
                                        Some(w),
                                        cx,
                                    );
                                    Self::tint(cx, dark);
                                    this.persist();
                                    cx.notify();
                                }))
                        }),
                ),
                p,
            ))
            .child(div().h(px(1.)).bg(rgb(p.line)))
            .child(setting_row(
                "发送方式",
                "Shift+Enter 始终换行。",
                div().flex().gap_2().children(
                    [(false, "Ctrl+Enter"), (true, "Enter")]
                        .into_iter()
                        .map(|(enter, title)| {
                            button(format!("send-mode-{enter}"), title, "corner-down-left")
                                .when(self.library.settings.send_on_enter == enter, |b| {
                                    b.bg(rgb(p.soft)).text_color(rgb(p.accent))
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.library.settings.send_on_enter = enter;
                                    this.composer
                                        .update(cx, |s, cx| s.set_submit_on_enter(enter, cx));
                                    this.persist();
                                    cx.notify();
                                }))
                        }),
                ),
                p,
            ))
            .child(div().h(px(1.)).bg(rgb(p.line)))
            .child(setting_row(
                "侧栏宽度",
                "拖动侧栏右侧的分隔线调整，宽度会自动保存。",
                button("reset-sidebar", "恢复默认宽度", "panel-left").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.library.settings.sidebar_width = 252.;
                        this.persist();
                        cx.notify();
                    },
                )),
                p,
            ))
            .child(div().h(px(1.)).bg(rgb(p.line)))
            .child(div().font_weight(FontWeight::MEDIUM).child("Bash 本机命令"))
            .child(label("默认工作目录为 ~（用户主目录），支持绝对路径或 ~/子目录。每次调用均从此目录开始。", p.muted))
            .child(label("Bash 可执行文件", p.muted))
            .child(Input::new(&self.bash_program).w_full())
            .child(label("默认工作目录", p.muted))
            .child(Input::new(&self.bash_directory).w_full())
            .child(label("未安装 Bash 时会跳过本机命令工具，聊天和其他工具仍可使用。Windows 可安装 Git Bash 后指定其完整路径。", p.muted))
            .child(button("save-bash-settings", "保存并检测 Bash", "check").on_click(cx.listener(|this, _, w, cx| {
                let program = this.bash_program.read(cx).value().trim().to_string();
                let directory = this.bash_directory.read(cx).value().trim().to_string();
                if program.is_empty() {
                    this.show_toast("请填写 Bash 可执行文件路径".into()); cx.notify(); return;
                }
                this.library.settings.bash_path = program;
                this.library.settings.bash_working_directory = if directory.is_empty() { "~".into() } else { directory };
                this.load_advanced(w, cx);
                this.persist();
                let settings = this.library.settings.clone();
                let tx = this.work_tx.clone();
                std::thread::spawn(move || {
                    let result = crate::tools::validate_bash(&settings).map(|()| "Bash 配置已保存，检测通过".to_string());
                    let _ = tx.send(Work::Notice(result));
                }); cx.notify();
            })))
            .child(label("其他配置自动保存；Bash 路径修改需点击保存按钮。", p.muted))
    }
    pub(super) fn add_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.library.providers.push(Provider::custom());
        self.load_provider(self.library.providers.len() - 1, window, cx);
        self.provider_picker = false;
        self.persist();
        cx.notify();
    }
    pub(super) fn remove_connection(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.library.providers.iter().position(|p| p.id == id) else {
            return;
        };
        let in_use = self.generation.as_ref().is_some_and(|g| {
            self.library
                .conversations
                .iter()
                .any(|c| c.id == g.conversation && c.provider_id == id)
        });
        if in_use || self.media_busy || self.recording.is_some() {
            self.show_toast("请先停止此连接的回复或音频任务，再删除连接".into());
            cx.notify();
            return;
        }
        // Test fixtures never access the real system credential store.
        if self.store.is_some()
            && let Err(error) = storage::set_api_key(&self.library.providers[index], "")
        {
            self.show_toast(format!("无法移除连接密钥：{error:#}"));
            cx.notify();
            return;
        }
        self.library.providers.remove(index);
        let selected = if index < self.selected_provider {
            self.selected_provider.saturating_sub(1)
        } else {
            self.selected_provider
        };
        self.load_provider(
            selected.min(self.library.providers.len().saturating_sub(1)),
            window,
            cx,
        );
        self.provider_picker = false;
        self.model_picker = false;
        self.persist();
        self.show_toast("连接已删除，已有聊天记录保留".into());
        cx.notify();
    }
    fn render_connections(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let selected = self.library.providers.get(self.selected_provider);
        if selected.is_none() {
            return card(p)
                .p_6()
                .gap_4()
                .child(icon("plug").size(px(28.)).text_color(rgb(p.accent)))
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr("添加你的第一个连接")),
                )
                .child(label(
                    "选择接口协议，填写地址和密钥，再获取或手动添加模型。",
                    p.muted,
                ))
                .child(
                    Button::new("first-connection")
                        .primary()
                        .label(tr("添加连接"))
                        .on_click(cx.listener(|this, _, w, cx| this.add_connection(w, cx))),
                );
        }
        let selected = selected.unwrap();
        let list = card(p)
            .p_3()
            .gap_1()
            .when(!self.compact, |d| d.w(px(188.)).flex_shrink_0())
            .child(
                div()
                    .px_2()
                    .py_2()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("模型连接")),
            )
            .when(self.compact, |d| {
                d.child(
                    button("provider-picker", selected.name.clone(), "chevrons-up-down")
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.provider_picker = !this.provider_picker;
                            cx.notify();
                        })),
                )
            })
            .when(!self.compact || self.provider_picker, |d| {
                d.children(
                    self.library
                        .providers
                        .iter()
                        .enumerate()
                        .map(|(i, provider)| {
                            navigation_button(
                                format!("provider-{}", provider.id),
                                provider.name.clone(),
                                if provider.protocol == Protocol::Ollama {
                                    "hard-drive"
                                } else {
                                    "cloud"
                                },
                            )
                            .when(i == self.selected_provider, |b| {
                                b.bg(rgb(p.soft)).text_color(rgb(p.accent))
                            })
                            .on_click(cx.listener(
                                move |this, _, w, cx| {
                                    this.load_provider(i, w, cx);
                                    this.provider_picker = false;
                                },
                            ))
                        }),
                )
            })
            .child(
                button("add-provider", "添加连接", "plus")
                    .w_full()
                    .on_click(cx.listener(|this, _, w, cx| this.add_connection(w, cx))),
            );
        div()
            .flex()
            .gap_4()
            .min_w_0()
            .when(self.compact, |d| d.flex_col())
            .child(list)
            .child(self.render_connection_form(p, cx).flex_1().min_w_0())
    }
    fn render_connection_form(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let provider = &self.library.providers[self.selected_provider];
        let protocol = provider.protocol.clone();
        if self.model_editor {
            return card(p).p_6().child(self.render_model_settings(p, cx));
        }
        card(p)
            .p_6()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        icon(if protocol == Protocol::Ollama {
                            "hard-drive"
                        } else {
                            "cloud"
                        })
                        .size(px(24.))
                        .text_color(rgb(p.accent)),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(provider.name.clone()),
                    )
                    .child(
                        button("delete-connection", "删除连接", "trash")
                            .flex_shrink_0()
                            .text_color(rgb(0xc15d48))
                            .tooltip(tr("移除连接和密钥，保留已有聊天"))
                            .on_click(cx.listener({
                                let id = provider.id.clone();
                                move |this, _, w, cx| this.remove_connection(&id, w, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .child(
                        field("连接名称", Input::new(&self.provider_name), p)
                            .flex_1()
                            .min_w(px(200.)),
                    )
                    .child(field(
                        "接口协议",
                        div().flex().flex_wrap().gap_1().children(
                            [
                                (Protocol::OpenAi, "OpenAI 兼容"),
                                (Protocol::Ollama, "Ollama"),
                                (Protocol::Anthropic, "Anthropic"),
                            ]
                            .into_iter()
                            .map(|(choice, name)| {
                                Button::new(format!("protocol-{name}"))
                                    .ghost()
                                    .small()
                                    .label(name)
                                    .when(choice == protocol, |b| {
                                        b.bg(rgb(p.soft)).text_color(rgb(p.accent))
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.library.providers[this.selected_provider].protocol =
                                            choice.clone();
                                        cx.notify();
                                    }))
                            }),
                        ),
                        p,
                    )),
            )
            .child(field(
                if protocol == Protocol::Ollama {
                    "API 地址（Ollama 原生：如 http://localhost:11434，无需 /v1）"
                } else {
                    "API 地址（含 /v1 等服务商前缀）"
                },
                Input::new(&self.provider_url),
                p,
            ))
            .child(field("API Key", Input::new(&self.provider_key), p))
            .when(protocol == Protocol::OpenAi, |d| {
                d.child(field(
                    "兼容后端",
                    div().flex().flex_wrap().gap_2().children(
                        [
                            (OpenAiBackend::Auto, "自动识别"),
                            (OpenAiBackend::Standard, "标准兼容"),
                            (OpenAiBackend::LlamaCpp, "llama.cpp"),
                            (OpenAiBackend::LmStudio, "LM Studio"),
                            (OpenAiBackend::Vllm, "vLLM / SGLang"),
                        ]
                        .into_iter()
                        .map(|(backend, title)| {
                            button(format!("backend-{backend:?}"), title, "server")
                                .when(provider.backend == backend, |b| {
                                    b.bg(rgb(p.soft)).text_color(rgb(p.accent))
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.library.providers[this.selected_provider].backend =
                                        backend;
                                    this.persist();
                                    cx.notify();
                                }))
                        }),
                    ),
                    p,
                ))
            })
            .child(self.render_model_settings(p, cx))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("save-provider")
                            .primary()
                            .label(tr("保存配置"))
                            .on_click(cx.listener(|this, _, w, cx| {
                                this.save_provider(w, cx);
                            })),
                    )
                    .child(
                        button("fetch-models", "连接并获取模型", "refresh-cw").on_click(
                            cx.listener(|this, _, w, cx| {
                                if this.save_provider(w, cx) {
                                    let p = this.library.providers[this.selected_provider].clone();
                                    let tx = this.work_tx.clone();
                                    this.show_toast("正在获取模型列表…".into());
                                    std::thread::spawn(move || {
                                        let result = provider::models(&p);
                                        let _ = tx.send(Work::Models(p.id, result));
                                    });
                                }
                            }),
                        ),
                    )
                    .child(
                        button(
                            "provider-enabled",
                            if provider.enabled {
                                "已启用"
                            } else {
                                "已停用"
                            },
                            "power",
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            let p = &mut this.library.providers[this.selected_provider];
                            p.enabled = !p.enabled;
                            this.persist();
                            cx.notify();
                        })),
                    ),
            )
            .child(
                button("clear-key", "移除已保存的密钥", "key-round").on_click(cx.listener(
                    |this, _, _, cx| {
                        let p = &this.library.providers[this.selected_provider];
                        this.show_toast(match storage::set_api_key(p, "") {
                            Ok(_) => "已移除系统凭据库中的密钥".into(),
                            Err(e) => format!("{e:#}"),
                        });
                        cx.notify();
                    },
                )),
            )
            .child(label(
                "模型能力由服务商和所选模型决定；视觉、PDF、工具、图片生成与语音接口需对应支持。",
                p.muted,
            ))
    }
    fn render_data(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                card(p)
                    .p_6()
                    .child(heading(
                        "你的数据，留在你的设备上。",
                        "会话和附件保存在本地，API Key 保存在系统凭据库。",
                        p,
                    ))
                    .when_some(self.store.as_ref(), |d, s| {
                        d.child(label(s.root.to_string_lossy().to_string(), p.muted))
                    })
                    .child(
                        button("open-storage", "打开数据目录", "folder-open").on_click(
                            cx.listener(|this, _, _, cx| {
                                if let Some(s) = &this.store {
                                    cx.open_url(&format!(
                                        "file:///{}",
                                        s.root.to_string_lossy().replace('\\', "/")
                                    ));
                                }
                            }),
                        ),
                    ),
            )
            .child(
                card(p)
                    .p_6()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("备份、导入与迁移")),
                    )
                    .child(label(
                        "JSON 导出包含会话和配置；完整备份还需要复制附件目录。",
                        p.muted,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                button("export-library", "导出所有会话", "download")
                                    .on_click(cx.listener(|this, _, _, _| this.export(true))),
                            )
                            .child(
                                button("import-library", "导入会话", "upload")
                                    .on_click(cx.listener(|this, _, _, _| this.import())),
                            ),
                    ),
            )
            .child(
                card(p)
                    .p_6()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(img(mori_logo()).size(px(44.)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Mori"))
                                    .child(label("原生 AI 聊天客户端", p.muted)),
                            ),
                    )
                    .child(label(
                        format!("Mori {} · gpui-kit 0.7.3", env!("CARGO_PKG_VERSION")),
                        p.muted,
                    )),
            )
    }
}
