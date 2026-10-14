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
            (&self.provider_name, "服务商名称"),
            (&self.provider_key, "新的 API Key（留空保留现有密钥）"),
            (&self.mcp_name, "例如：我的资料库"),
            (&self.mcp_command, "例如：npx.cmd 或程序的完整路径"),
            (&self.mcp_url, "https://example.com/mcp（本地程序留空）"),
        ] {
            input.update(cx, |s, cx| s.set_placeholder(tr(text), window, cx));
        }
        for (input, text) in [
            (&self.composer, "有什么想聊的？也可以附上图片或文档…"),
            (&self.provider_models, "每行一个模型 ID"),
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
            .child(label("配置已自动保存", p.muted))
    }
    fn render_connections(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let selected = &self.library.providers[self.selected_provider];
        let list = card(p)
            .p_3()
            .gap_1()
            .when(!self.compact, |d| d.w(px(188.)).flex_shrink_0())
            .child(
                div()
                    .px_2()
                    .py_2()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("服务商")),
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
                button("add-provider", "添加兼容服务商", "plus")
                    .w_full()
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.library.providers.push(Provider {
                            id: id(),
                            name: tr("自定义服务商"),
                            protocol: Protocol::OpenAi,
                            base_url: "https://api.example.com/v1".into(),
                            models: vec![],
                            enabled: true,
                        });
                        this.load_provider(this.library.providers.len() - 1, w, cx);
                        this.persist();
                    })),
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
                            .child(provider.name.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .child(
                        field("服务商名称", Input::new(&self.provider_name), p)
                            .flex_1()
                            .min_w(px(200.)),
                    )
                    .child(field(
                        "接口协议",
                        div().flex().flex_wrap().gap_1().children(
                            [
                                (Protocol::OpenAi, "OpenAI"),
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
            .child(field(
                "模型 ID（可手动填写，每行一个）",
                Textarea::new(&self.provider_models),
                p,
            ))
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
                    .child(label("Mori 0.1.0 · gpui-kit 0.7.3", p.muted)),
            )
    }
}
