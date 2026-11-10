use super::app::Wife;
use super::style::{self, draw_bubble};
use eframe::egui::{self, Color32, Rect, vec2};
use eframe::egui::{Sense, Stroke};

pub(super) enum MessageSource<'a> {
    Main,
    Development,
    External(&'a crate::backend::handoff::Request),
}

impl Wife {
    pub(super) fn submit(&mut self, ctx: &egui::Context) {
        if self.input.trim().is_empty() || self.busy {
            return;
        }
        self.send_message(ctx, self.input.trim().to_owned(), MessageSource::Main);
    }
    #[cfg(any(target_os = "linux", windows))]
    pub(super) fn poll_handoff(&mut self, ctx: &egui::Context) {
        if !self.busy
            && let Some(request) = self.inbox.as_ref().and_then(|inbox| inbox.poll())
        {
            self.last_activity = std::time::Instant::now();
            let route = if request.workspace.is_none() && self.development.book.projects.is_empty()
            {
                Ok(None)
            } else {
                self.development
                    .book
                    .route(request.workspace.as_deref(), &request.files)
            };
            match route {
                Ok(Some(path)) => {
                    if let Err(error) = self.switch_development_workspace(Some(path), ctx) {
                        self.error = error;
                        return;
                    }
                }
                Ok(None) => {
                    if self.development.book.active.is_some() {
                        self.error = "来源未指定项目，未发送到当前项目；请先切换普通会话。".into();
                        self.text = self.error.clone();
                        return;
                    }
                }
                Err(error) => {
                    self.error = error;
                    self.text = self.error.clone();
                    self.full_text = true;
                    return;
                }
            }
            if let Some(mut snapshot) = request.diagnostics.clone()
                && let Some(project) = self.development.book.current_mut()
            {
                if let Err(error) = snapshot.capture(&project.path) {
                    self.error = error;
                    self.text = self.error.clone();
                    return;
                }
                if let Some(client) = &self.codex {
                    snapshot.redact(|text| client.sanitize(text));
                }
                project.diagnostics = Some(snapshot);
                self.development.diagnostics_ui.refresh = true;
                self.save_development(true);
                if request.text.trim().is_empty() && request.files.is_empty() {
                    self.development.open = true;
                    self.development.page = 1;
                    return;
                }
            }
            self.send_message(ctx, request.prompt(), MessageSource::External(&request));
            if !self.error.is_empty() {
                self.text = self.error.clone();
                self.full_text = true;
            }
        }
    }
    pub(super) fn send_message(
        &mut self,
        ctx: &egui::Context,
        text: String,
        source: MessageSource<'_>,
    ) {
        if self.busy {
            self.error = "当前任务仍在运行。".into();
            return;
        }
        let memory_plan = if self.config.codex.development_mode {
            self.development
                .book
                .current()
                .map(|project| {
                    crate::development::memory::context(&project.path, &project.memories)
                })
                .unwrap_or_default()
        } else {
            Default::default()
        };
        if self.config.codex.development_mode && memory_plan.0 != self.development.memory_context {
            self.new_development_task(ctx);
        }
        self.development.memory_ui.report = memory_plan.1;
        self.development.diagnostics_ui.refresh = true;
        let context = if self.config.codex.development_mode {
            self.development
                .book
                .current()
                .map(|project| {
                    let mut parts = vec![self.development.memory_context.clone()];
                    if let Some(snapshot) = &project.diagnostics {
                        parts.push(snapshot.context());
                    }
                    let mut tests = String::from(
                        "[登记的测试命令，仅供选择，未自动运行；历史结果不代表当前代码]\n",
                    );
                    for definition in &project.tests {
                        let line =
                            format!("{}: {}\n", definition.name, definition.display_command());
                        if tests.chars().count() + line.chars().count() > 6000 {
                            break;
                        }
                        tests.push_str(&line);
                    }
                    if !project.tests.is_empty() {
                        parts.push(tests);
                    }
                    parts
                        .into_iter()
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n\n")
                })
                .filter(|context| !context.is_empty())
        } else {
            None
        };
        if self.codex_restart_pending && !self.busy {
            self.start_codex(ctx);
        }
        self.development_turn = matches!(source, MessageSource::Development);
        let external = match source {
            MessageSource::External(request) => Some(request),
            _ => None,
        };
        let remembered_input = self
            .codex
            .as_ref()
            .map_or_else(|| text.clone(), |client| client.sanitize(&text));
        let result = self
            .codex
            .as_ref()
            .ok_or_else(|| "Codex 未启动，请在设置中重启会话。".to_owned())
            .and_then(|codex| {
                codex.submit_with_context(text, self.config.codex.no_approval, context)
            });
        match result {
            Ok(()) => {
                let display = external.map_or_else(
                    || remembered_input.clone(),
                    |request| {
                        let mut display = request.text.trim().to_owned();
                        if !request.source.is_empty() {
                            display.push_str(&format!("\n\n来源：{}", request.source));
                        }
                        for file in &request.files {
                            display.push_str(&format!("\n文件：{file}"));
                        }
                        if let Some(line) = request.line {
                            display.push_str(&format!("\n起始行：{line}"));
                        }
                        if let Some(client) = &self.codex {
                            client.sanitize(display.trim())
                        } else {
                            display
                        }
                    },
                );
                self.record_development_start(display);
                self.context_ui.last_input = remembered_input;
                self.busy = true;
                self.turn_ui_hidden = false;
                self.dialog_clicks = 0;
                self.external_turn = external.is_some();
                self.codex_status = "Lexington正在思考".into();
                self.error.clear();
                self.codex_messages.clear();
                self.text.clear();
            }
            Err(error) => self.error = error,
        }
        ctx.request_repaint();
    }
    pub(super) fn input_ui(
        &mut self,
        ctx: &egui::Context,
        screen: Rect,
        center: egui::Pos2,
        image_height: f32,
    ) {
        if self.turn_ui_hidden || (self.busy && self.development_turn) {
            self.input_bounds = None;
            ctx.memory_mut(|memory| memory.surrender_focus("message-input".into()));
            return;
        }
        let width = self.config.dialog.width;
        let error_height = if self.error.is_empty() || self.busy || self.development_turn {
            0.
        } else {
            ctx.fonts(|fonts| {
                let mut job = egui::text::LayoutJob::simple(
                    self.error.clone(),
                    style::font(13.),
                    Color32::from_rgb(231, 76, 60),
                    width - 28.,
                );
                job.sections[0].format.line_height = Some(19.5);
                fonts.layout_job(job).size().y.clamp(19.5, 100.)
            })
        };
        let height = 50.
            + if error_height > 0. {
                error_height + 8.
            } else {
                0.
            };
        // Preserve the old 450px component's local boundary checks before translation.
        let origin = center - vec2(225., 225.);
        let mut position = origin
            + vec2(
                (225. - width / 2.).clamp(10., (screen.width() - 10. - width).max(10.)),
                (225. + image_height / 2. + self.config.dialog.offset_y).clamp(
                    10.,
                    (screen.height() - 10. - self.config.dialog.max_height).max(10.),
                ),
            );
        let id = egui::Id::new("message-input");
        let input_transform = ctx
            .layer_transform_to_global(egui::LayerId::new(egui::Order::Foreground, "input".into()))
            .unwrap_or_default();
        // Keep the full editor/error panel inside the native viewport after hover scaling.
        let global = input_transform * Rect::from_min_size(position, vec2(width, height));
        let top = global.top().clamp(
            screen.top() + 4.,
            (screen.bottom() - global.height() - 4.).max(screen.top() + 4.),
        );
        position = input_transform.inverse() * egui::pos2(global.left(), top);
        let rect = Rect::from_min_size(position, vec2(width, height));
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        #[cfg(any(target_os = "linux", windows))]
        // Transparent areas receive no native motion events. Poll for hover
        // discovery without claiming clicks or stealing another app's focus.
        let pointer = self.drag_window.as_ref().map_or(pointer, |window| {
            window.pointer_position(ctx.pixels_per_point()).or(pointer)
        });
        let hovered = pointer.is_some_and(|p| {
            (input_transform * rect).contains(p)
                || (input_transform * Rect::from_center_size(center, vec2(450., 450.))).contains(p)
        });
        self.input_bounds = Some(input_transform * rect);
        // egui can retain a TextEdit focus ID after another native window gains
        // focus. It must not keep the pet's input visible (or consume keys).
        if !ctx.input(|i| i.focused) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        let focused = ctx.memory(|m| m.has_focus(id));
        let alpha = ctx.animate_value_with_time(
            egui::Id::new("input-alpha"),
            if hovered || focused || self.busy {
                1.
            } else {
                0.
            },
            0.3,
        );
        if alpha > 0.01 {
            self.input_regions.push(input_transform * rect);
        }
        egui::Area::new("input".into())
            .constrain(false)
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                let rect = Rect::from_min_size(ui.min_rect().min, rect.size());
                draw_bubble(ui, rect, &mut self.input_bg, true);
                ui.set_min_size(rect.size());
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(rect.shrink2(vec2(14., 10.))),
                    |ui| {
                        if self.busy {
                            ui.horizontal_centered(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.;
                                let (rect, _) =
                                    ui.allocate_exact_size(vec2(16., 16.), Sense::hover());
                                let center = rect.center();
                                let points: Vec<_> = (0..=32)
                                    .map(|n| {
                                        let angle = n as f32 / 32. * std::f32::consts::TAU;
                                        center + vec2(angle.cos(), angle.sin()) * 7.
                                    })
                                    .collect();
                                ui.painter().add(egui::Shape::line(
                                    points,
                                    Stroke::new(2_f32, Color32::from_black_alpha(26)),
                                ));
                                let angle =
                                    ui.input(|i| i.time) as f32 * std::f32::consts::TAU / 0.8;
                                let arc: Vec<_> = (0..=8)
                                    .map(|n| {
                                        let a = angle + n as f32 / 8. * std::f32::consts::FRAC_PI_2;
                                        center + vec2(a.cos(), a.sin()) * 7.
                                    })
                                    .collect();
                                ui.painter().add(egui::Shape::line(
                                    arc,
                                    Stroke::new(2_f32, Color32::from_gray(51)),
                                ));
                                ui.label(
                                    egui::RichText::new("Lexington正在思考")
                                        .font(style::font(13.))
                                        .color(Color32::from_gray(102)),
                                );
                            });
                        } else {
                            let edit = ui.add_sized(
                                vec2(width - 28., 30.),
                                egui::TextEdit::singleline(&mut self.input)
                                    .id(id)
                                    .desired_width(width - 28.)
                                    .font(style::font(14.))
                                    .text_color(Color32::from_gray(51))
                                    .hint_text(
                                        egui::RichText::new("输入消息...")
                                            .color(Color32::from_gray(153)),
                                    )
                                    .margin(egui::Margin::ZERO)
                                    .vertical_align(egui::Align::Center)
                                    .frame(false),
                            );
                            if error_height > 0. {
                                // Errors accompany the editor, so a failed connection cannot eat later keystrokes.
                                egui::ScrollArea::vertical()
                                    .id_salt("input-error")
                                    .max_height(error_height)
                                    .drag_to_scroll(false)
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&self.error)
                                                    .font(style::font(13.))
                                                    .line_height(Some(19.5))
                                                    .color(Color32::from_rgb(231, 76, 60)),
                                            )
                                            .wrap(),
                                        );
                                    });
                            }
                            if (edit.has_focus() || edit.lost_focus())
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                self.submit(ctx);
                            }
                        }
                    },
                );
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.busy {
                if let Some(codex) = &self.codex {
                    codex.interrupt();
                }
                self.codex_status = "正在停止…".into();
            } else {
                self.input.clear();
                self.error.clear();
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
        }
    }
}
