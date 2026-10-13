use super::*;

impl Mori {
    pub(super) fn render_extensions(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let settings = &self.library.settings;
        let controls = div()
            .flex()
            .gap_4()
            .child(
                card(p)
                    .flex_1()
                    .child(icon("blocks").size(px(28.)).text_color(rgb(p.accent)))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("连接外部能力"),
                    )
                    .child(label("让助手查资料、使用你连接的服务", p.muted))
                    .child(
                        button(
                            "enable-tools",
                            if settings.tools_enabled {
                                "已启用，点击关闭"
                            } else {
                                "启用工具"
                            },
                            "power",
                        )
                        .w(px(168.))
                        .on_click(cx.listener(|this, _, w, cx| {
                            this.library.settings.tools_enabled =
                                !this.library.settings.tools_enabled;
                            this.load_advanced(w, cx);
                            this.persist();
                            cx.notify();
                        })),
                    ),
            )
            .child(
                card(p)
                    .flex_1()
                    .child(icon("terminal").size(px(28.)).text_color(rgb(p.accent)))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("本机命令"))
                    .child(label("需要时才开启，每次执行前确认", p.muted))
                    .child(
                        button(
                            "enable-bash",
                            if settings.bash_enabled {
                                "Bash 已启用"
                            } else {
                                "启用 Bash"
                            },
                            "power",
                        )
                        .w(px(144.))
                        .on_click(cx.listener(|this, _, w, cx| {
                            this.library.settings.bash_enabled =
                                !this.library.settings.bash_enabled;
                            this.load_advanced(w, cx);
                            this.persist();
                            cx.notify();
                        })),
                    ),
            );
        let servers = card(p)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("已连接的服务"),
            )
            .when(settings.mcp.is_empty(), |d| {
                d.child(label(
                    "还没有连接服务。添加一个 MCP 服务，让助手访问你的资料与工具。",
                    p.muted,
                ))
            })
            .children(settings.mcp.iter().enumerate().map(|(index, server)| {
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(p.line))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(icon("plug").text_color(rgb(p.accent)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_sm().child(server.name.clone()))
                                    .child(label(
                                        if server.url.is_empty() {
                                            "本地服务"
                                        } else {
                                            "远程服务"
                                        },
                                        p.muted,
                                    )),
                            ),
                    )
                    .child(
                        button(
                            format!("mcp-toggle-{index}"),
                            if server.enabled {
                                "已启用"
                            } else {
                                "已停用"
                            },
                            "power",
                        )
                        .on_click(cx.listener(move |this, _, w, cx| {
                            let s = &mut this.library.settings.mcp[index];
                            s.enabled = !s.enabled;
                            this.load_advanced(w, cx);
                            this.persist();
                            cx.notify();
                        })),
                    )
            }));
        let add = card(p)
            .child(div().font_weight(FontWeight::SEMIBOLD).child("添加服务"))
            .child(field("给它起个名字", Input::new(&self.mcp_name), p))
            .child(field("远程服务地址", Input::new(&self.mcp_url), p))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(field("或者，启动本地程序", Input::new(&self.mcp_command), p).flex_1())
                    .child(
                        field("启动参数（每行一个）", Textarea::new(&self.mcp_args), p).flex_1(),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(label("支持 MCP stdio 和 Streamable HTTP", p.muted))
                    .child(
                        Button::new("mcp-add")
                            .primary()
                            .label("添加连接")
                            .icon(icon("plus"))
                            .on_click(cx.listener(|this, _, w, cx| {
                                let name = this.mcp_name.read(cx).value().trim().to_string();
                                let url = this.mcp_url.read(cx).value().trim().to_string();
                                let command = this.mcp_command.read(cx).value().trim().to_string();
                                if name.is_empty() || (url.is_empty() && command.is_empty()) {
                                    this.toast = Some("请填写名称，以及远程地址或本地程序".into());
                                    cx.notify();
                                    return;
                                }
                                if !url.is_empty()
                                    && !(url.starts_with("https://") || url.starts_with("http://"))
                                {
                                    this.toast = Some("远程服务地址需使用 HTTP 或 HTTPS".into());
                                    cx.notify();
                                    return;
                                }
                                let args = this
                                    .mcp_args
                                    .read(cx)
                                    .value()
                                    .lines()
                                    .filter(|s| !s.trim().is_empty())
                                    .map(str::to_string)
                                    .collect();
                                this.library.settings.mcp.push(McpConfig {
                                    name,
                                    url,
                                    command,
                                    args,
                                    env: Default::default(),
                                    enabled: true,
                                });
                                this.load_advanced(w, cx);
                                this.persist();
                                this.toast = Some(
                                    "连接已添加。开启工具后，聊天时会自动发现可用能力。".into(),
                                );
                                cx.notify();
                            })),
                    ),
            );
        let generation = card(p)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("让回答更合心意"),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(field("创造性（0–2）", Input::new(&self.temperature), p).flex_1())
                    .child(field("最长输出（tokens）", Input::new(&self.max_tokens), p).flex_1()),
            )
            .child(field("图片生成模型", Input::new(&self.image_model), p))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(field("朗读模型", Input::new(&self.speech_model), p).flex_1())
                    .child(
                        field("语音转写模型", Input::new(&self.transcription_model), p).flex_1(),
                    ),
            )
            .child(
                Button::new("save-generation")
                    .primary()
                    .label("保存偏好")
                    .w(px(140.))
                    .on_click(cx.listener(|this, _, w, cx| {
                        let temp = this.temperature.read(cx).value().parse::<f64>();
                        let max = this.max_tokens.read(cx).value().parse::<u32>();
                        match (temp, max) {
                            (Ok(temp), Ok(max))
                                if temp.is_finite() && (0.0..=2.0).contains(&temp) && max > 0 =>
                            {
                                this.library.settings.temperature = temp;
                                this.library.settings.max_tokens = max;
                                this.library.settings.image_model =
                                    this.image_model.read(cx).value().trim().to_string();
                                this.library.settings.speech_model =
                                    this.speech_model.read(cx).value().trim().to_string();
                                this.library.settings.transcription_model =
                                    this.transcription_model.read(cx).value().trim().to_string();
                                this.load_advanced(w, cx);
                                this.persist();
                                this.toast = Some("偏好已保存".into());
                            }
                            _ => this.toast = Some("创造性须为 0–2，最长输出须为正整数".into()),
                        }
                        cx.notify();
                    })),
            );
        div().id("tools-page").size_full().max_w(px(960.)).mx_auto().overflow_y_scroll().p_8().flex().flex_col().gap_6()
            .child(heading("让对话，多一点可能。", "连接外部资料和服务。工具由模型提出请求，每次执行前都由你确认。", p))
            .child(controls).child(servers).child(add).child(generation)
            .child(button("advanced-toggle", if self.show_advanced { "收起高级设置" } else { "高级设置与自定义函数" }, "sliders-horizontal")
                .on_click(cx.listener(|this, _, w, cx| { this.show_advanced = !this.show_advanced; if this.show_advanced { this.load_advanced(w, cx); } cx.notify(); })))
            .when(self.show_advanced, |d| d.child(card(p).child(label("可配置 Bash 路径、MCP 环境变量与自定义函数。", p.muted))
                .child(Textarea::new(&self.advanced).font_family("Cascadia Code").text_sm())
                .child(Button::new("save-advanced").primary().label("验证并保存").on_click(cx.listener(|this, _, w, cx| {
                    match serde_json::from_str::<Settings>(&this.advanced.read(cx).value()) {
                        Ok(s) if s.temperature.is_finite() && (0.0..=2.0).contains(&s.temperature) && s.max_tokens > 0 => {
                            this.library.settings = s;
                            Theme::change(if this.library.settings.dark { ThemeMode::Dark } else { ThemeMode::Light }, Some(w), cx); Self::tint(cx, this.library.settings.dark);
                            this.temperature.update(cx, |s,cx| s.set_value(this.library.settings.temperature.to_string(),w,cx));
                            this.max_tokens.update(cx, |s,cx| s.set_value(this.library.settings.max_tokens.to_string(),w,cx));
                            this.image_model.update(cx, |s,cx| s.set_value(this.library.settings.image_model.clone(),w,cx));
                            this.speech_model.update(cx, |s,cx| s.set_value(this.library.settings.speech_model.clone(),w,cx));
                            this.transcription_model.update(cx, |s,cx| s.set_value(this.library.settings.transcription_model.clone(),w,cx));
                            this.persist(); this.toast = Some("扩展设置已保存".into());
                        }, Ok(_) => this.toast = Some("温度须为 0–2，max_tokens 须大于 0".into()), Err(e) => this.toast = Some(format!("JSON 格式错误：{e}")),
                    } cx.notify();
                })))
                .child(label("函数参数 JSON 通过 stdin 交给 command，程序输出返回模型。每次调用都需要确认。", p.muted))))
    }
}
