#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod assets;
mod backend;
mod platform;
mod visual;

use assets::Picture;
use backend::{Config, Illustration};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, Vec2, pos2, vec2};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

fn main() -> eframe::Result {
    let dev = std::env::args().any(|arg| arg == "--dev");
    let size = if dev { [800., 600.] } else { [500., 500.] };
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(size)
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(dev)
        .with_always_on_top()
        .with_taskbar(false)
        .with_title("Lexington 桌面宠物");
    if dev {
        viewport = viewport.with_position([100., 100.]);
    }
    if let Ok(icon) = image::open(backend::resource_root().join("resource/icon/Normal.ico")) {
        let icon = icon.into_rgba8();
        viewport = viewport.with_icon(egui::IconData {
            width: icon.width(),
            height: icon.height(),
            rgba: icon.into_raw(),
        });
    }
    let mut options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        persist_window: false,
        ..Default::default()
    };
    // X11/XWayland supports desktop-pet placement, taskbar hiding and always-on-top.
    // Native Wayland delegates those policies to its compositor.
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_some() && !std::env::args().any(|a| a == "--wayland") {
        use winit::platform::x11::EventLoopBuilderExtX11;
        options.window_builder = Some(Box::new(|attributes| {
            attributes.with_window_type(egui::X11WindowType::Utility)
        }));
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_x11();
        }));
    }
    eframe::run_native(
        "Lexington",
        options,
        Box::new(|cc| Ok(Box::new(Pet::new(cc)))),
    )
}

struct Pet {
    #[cfg(target_os = "linux")]
    drag_window: Option<platform::DragWindow>,
    root: PathBuf,
    config: Config,
    illustrations: Vec<Illustration>,
    selected: Option<usize>,
    character: String,
    format: String,
    picture: Option<Picture>,
    icons: Vec<egui::TextureHandle>,
    dialog_bg: Option<Picture>,
    input_bg: Option<Picture>,
    input_bounds: Option<Rect>,
    dialog_bounds: Option<Rect>,
    texts: Vec<String>,
    pinned: bool,
    text: String,
    typed_at: Instant,
    full_text: bool,
    dialog_clicks: u8,
    pet_pressed: Option<Instant>,
    drag_sent: bool,
    last_activity: Instant,
    clock_started: Option<Instant>,
    input: String,
    error: String,
    response: Option<mpsc::Receiver<Result<String, String>>>,
    capture: Option<PathBuf>,
    capture_requested: bool,
    capture_delay: Duration,
    started: Instant,
    placed: bool,
    raised: bool,
    hints_applied: bool,
    about: bool,
    settings: bool,
}
impl Pet {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        platform::configure(cc);
        let pinned = cc
            .storage
            .and_then(|s| s.get_string("sidebarAlwaysVisible"))
            .is_none_or(|s| s != "false");
        let mut app = Self::from_context(&cc.egui_ctx, pinned);
        #[cfg(target_os = "linux")]
        {
            app.drag_window = platform::DragWindow::new(cc);
        }
        app
    }
    fn from_context(ctx: &egui::Context, pinned: bool) -> Self {
        Self::from_root(ctx, pinned, backend::resource_root())
    }
    fn from_root(ctx: &egui::Context, pinned: bool, root: PathBuf) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        let mut cjk = egui::FontData::from_static(include_bytes!(
            "../resource/fonts/NotoSansCJK-Regular.ttc"
        ));
        cjk.index = 2; // Simplified Chinese face, matching the original renderer on Linux.
        fonts.font_data.insert("cjk".into(), cjk.into());
        fonts.font_data.insert(
            "serif".into(),
            egui::FontData::from_static(include_bytes!(
                "../resource/fonts/LiberationSerif-Regular.ttf"
            ))
            .into(),
        );
        fonts.font_data.insert(
            "arial".into(),
            egui::FontData::from_static(include_bytes!(
                "../resource/fonts/LiberationSans-Regular.ttf"
            ))
            .into(),
        );
        fonts.font_data.insert(
            "math".into(),
            egui::FontData::from_static(include_bytes!(
                "../resource/fonts/NotoSansMath-Regular.ttf"
            ))
            .into(),
        );
        fonts.families.insert(
            egui::FontFamily::Name("browser".into()),
            vec!["serif".into(), "cjk".into()],
        );
        fonts.families.insert(
            egui::FontFamily::Name("arial".into()),
            vec!["arial".into(), "cjk".into(), "math".into()],
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push("cjk".into());
        }
        ctx.set_fonts(fonts);
        ctx.set_visuals(egui::Visuals::light());
        let config = Config::load(&root);
        let mut app = Self {
            #[cfg(target_os = "linux")]
            drag_window: None,
            root,
            config,
            illustrations: vec![],
            selected: None,
            character: String::new(),
            format: "gif".into(),
            picture: None,
            icons: [
                include_bytes!("../resource/icon/pin.png").as_slice(),
                include_bytes!("../resource/icon/unpin.png").as_slice(),
                include_bytes!("../resource/icon/character.png").as_slice(),
                include_bytes!("../resource/icon/clock.png").as_slice(),
            ]
            .iter()
            .enumerate()
            .map(|(n, bytes)| {
                let image = image::load_from_memory(bytes)
                    .expect("内置图标")
                    .into_rgba8();
                ctx.load_texture(
                    format!("icon-{n}"),
                    egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    ),
                    egui::TextureOptions::LINEAR,
                )
            })
            .collect(),
            dialog_bg: None,
            input_bg: None,
            input_bounds: None,
            dialog_bounds: None,
            texts: vec![],
            pinned,
            text: String::new(),
            typed_at: Instant::now(),
            full_text: false,
            dialog_clicks: 0,
            pet_pressed: None,
            drag_sent: false,
            last_activity: Instant::now(),
            clock_started: None,
            input: String::new(),
            error: String::new(),
            response: None,
            capture: std::env::args()
                .skip_while(|a| a != "--screenshot")
                .nth(1)
                .map(PathBuf::from),
            capture_requested: false,
            capture_delay: Duration::from_secs_f64(
                std::env::args()
                    .skip_while(|arg| arg != "--screenshot-delay")
                    .nth(1)
                    .and_then(|s| s.parse::<f64>().ok())
                    .filter(|s| s.is_finite() && *s >= 0.)
                    .unwrap_or(4.),
            ),
            started: Instant::now(),
            placed: false,
            raised: false,
            hints_applied: false,
            about: false,
            settings: false,
        };
        app.refresh(ctx);
        app
    }
    fn characters(&self) -> Vec<String> {
        let mut names: Vec<_> = self
            .illustrations
            .iter()
            .map(|i| i.character.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }
    fn filtered(&self) -> Vec<usize> {
        self.illustrations
            .iter()
            .enumerate()
            .filter(|(_, i)| i.character == self.character && i.format == self.format)
            .map(|(n, _)| n)
            .collect()
    }
    fn select(&mut self, ctx: &egui::Context, index: Option<usize>) {
        self.selected = index;
        self.picture = index.and_then(|i| {
            Picture::load(ctx, &self.illustrations[i].path)
                .map_err(|e| eprintln!("加载立绘失败: {e}"))
                .ok()
        });
    }
    fn select_first(&mut self, ctx: &egui::Context) {
        if self.filtered().is_empty() {
            let other = if self.format == "gif" { "png" } else { "gif" };
            if self
                .illustrations
                .iter()
                .any(|ill| ill.character == self.character && ill.format == other)
            {
                self.format = other.into();
            }
        }
        self.select(ctx, self.filtered().first().copied());
    }
    fn refresh(&mut self, ctx: &egui::Context) {
        let old = self.selected.map(|i| self.illustrations[i].path.clone());
        self.config = Config::load(&self.root);
        self.illustrations = backend::illustrations(&self.root);
        let names = self.characters();
        if !names.contains(&self.character) {
            self.character = names.first().cloned().unwrap_or_default();
        }
        if let Some(i) =
            old.and_then(|path| self.illustrations.iter().position(|ill| ill.path == path))
        {
            self.select(ctx, Some(i));
        } else {
            self.select_first(ctx);
        }
        self.texts = backend::texts(&self.root, &self.character);
        self.dialog_bg = backend::background(&self.root, "dialog")
            .and_then(|path| Picture::load(ctx, &path).ok());
        self.input_bg = backend::background(&self.root, "input")
            .and_then(|path| Picture::load(ctx, &path).ok());
    }
    fn say(&mut self, text: String) {
        self.text = text;
        self.typed_at = Instant::now();
        self.last_activity = Instant::now();
        self.full_text = false;
        self.dialog_clicks = 0;
    }
    fn click_pet(&mut self) {
        if self.text.is_empty() {
            if !self.texts.is_empty() {
                self.say(self.texts[rand::random_range(0..self.texts.len())].clone());
            }
        } else if self.dialog_clicks == 0 {
            self.full_text = true;
            self.dialog_clicks = 1;
        } else {
            self.text.clear();
            self.dialog_clicks = 0;
        }
    }
    fn clock(&mut self) {
        let message = if self.clock_started.take().is_some() {
            self.config.clock_in.clock_in_cancel_message.clone()
        } else {
            self.clock_started = Some(Instant::now());
            self.config.clock_in.clock_in_message.clone()
        };
        self.say(message);
    }
    fn submit(&mut self, ctx: &egui::Context) {
        if self.input.trim().is_empty() || self.response.is_some() {
            return;
        }
        let config = Config::load(&self.root).llm;
        let input = self.input.clone();
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::channel();
        self.response = Some(receiver);
        self.error.clear();
        std::thread::spawn(move || {
            let _ = sender.send(backend::call_llm(&config, &input));
            ctx.request_repaint();
        });
    }
    fn context_menu(&mut self, ctx: &egui::Context, response: &egui::Response) {
        response.context_menu(|ui| {
            if ui.button("设置").clicked() {
                self.settings = true;
                ui.close_menu();
            }
            ui.separator();
            if ui.button("关于").clicked() {
                self.about = true;
                ui.close_menu();
            }
            ui.separator();
            if ui.button("关闭").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
    fn sidebar(&mut self, ctx: &egui::Context, screen: Rect) -> bool {
        let mut handled = false;
        let x = if screen.width() / 2. - 315. < 0. {
            10.
        } else {
            screen.width() / 2. - 315.
        };
        let bounds = Rect::from_min_size(pos2(x, screen.center().y - 155.), vec2(50., 310.));
        let hovered = ctx.input(|i| i.pointer.hover_pos().is_some_and(|p| bounds.contains(p)));
        let alpha = ctx.animate_value_with_time(
            egui::Id::new("sidebar-alpha"),
            if self.pinned || hovered { 1. } else { 0.2 },
            0.3,
        );
        egui::Area::new("sidebar".into())
            .fixed_pos(bounds.min + vec2(5., 10.))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(0., 10.);
                ui.set_opacity(alpha);
                let labels = [
                    if self.pinned { "📌" } else { "📍" },
                    "↻",
                    "▶",
                    if self.format == "gif" { "G" } else { "P" },
                    "👤",
                    "⏰",
                ];
                let tips = [
                    "切换侧边栏显示模式",
                    "刷新列表",
                    "切换到下一个立绘",
                    "切换 GIF/PNG 格式",
                    "切换角色",
                    if self.clock_started.is_some() {
                        "点击终止计时"
                    } else {
                        "上班打卡"
                    },
                ];
                for n in 0..6 {
                    let enabled = !((n == 2 || n == 4) && self.illustrations.is_empty());
                    let color = if n == 0 {
                        Color32::from_rgba_unmultiplied(150, 100, 255, 204)
                    } else if n == 5 && self.clock_started.is_some() {
                        Color32::from_rgba_unmultiplied(76, 175, 80, 204)
                    } else {
                        Color32::from_rgba_unmultiplied(100, 150, 255, 204)
                    };
                    let response = ui
                        .allocate_response(
                            vec2(40., 40.),
                            if enabled {
                                Sense::click()
                            } else {
                                Sense::hover()
                            },
                        )
                        .on_hover_text(tips[n]);
                    let scale = ctx.animate_value_with_time(
                        egui::Id::new(("button-scale", n)),
                        if response.is_pointer_button_down_on() {
                            0.95
                        } else if response.hovered() && enabled {
                            1.1
                        } else {
                            1.
                        },
                        0.2,
                    );
                    let rect =
                        Rect::from_center_size(response.rect.center(), vec2(40., 40.) * scale);
                    let painter = ui.painter();
                    let color = if response.hovered() && enabled {
                        Color32::from_rgb(color.r(), color.g(), color.b())
                    } else {
                        color
                    };
                    let opacity = if enabled { 1. } else { 0.5 };
                    draw_shadow(painter, rect, 8., 8., (51. * opacity) as u8);
                    painter.rect_filled(rect, 8, color.gamma_multiply(opacity));
                    match n {
                        0 | 4 | 5 => {
                            let icon = if n == 0 {
                                usize::from(!self.pinned)
                            } else if n == 4 {
                                2
                            } else {
                                3
                            };
                            painter.image(
                                self.icons[icon].id(),
                                Rect::from_center_size(rect.center(), vec2(22.5, 21.25) * scale),
                                Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                                Color32::WHITE.gamma_multiply(opacity),
                            );
                        }
                        _ => {
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                labels[n],
                                FontId::new(18. * scale, egui::FontFamily::Name("arial".into())),
                                Color32::WHITE.gamma_multiply(opacity),
                            );
                        }
                    }
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(if enabled {
                            egui::CursorIcon::PointingHand
                        } else {
                            egui::CursorIcon::NotAllowed
                        });
                    }
                    if response.clicked() {
                        handled = true;
                        match n {
                            0 => self.pinned = !self.pinned,
                            1 => self.refresh(ctx),
                            2 => {
                                let indices = self.filtered();
                                let current = indices
                                    .iter()
                                    .position(|i| Some(*i) == self.selected)
                                    .unwrap_or(0);
                                self.select(
                                    ctx,
                                    indices.get((current + 1) % indices.len().max(1)).copied(),
                                );
                            }
                            3 => {
                                self.format =
                                    if self.format == "gif" { "png" } else { "gif" }.into();
                                self.select_first(ctx);
                            }
                            4 => {
                                let names = self.characters();
                                let index =
                                    names.iter().position(|s| s == &self.character).unwrap_or(0);
                                if !names.is_empty() {
                                    self.character = names[(index + 1) % names.len()].clone();
                                    self.texts = backend::texts(&self.root, &self.character);
                                    self.select_first(ctx);
                                }
                            }
                            5 => self.clock(),
                            _ => {}
                        }
                    }
                }
            });
        handled
    }
    fn bubble(&mut self, ctx: &egui::Context, center: egui::Pos2) -> Option<Rect> {
        if self.text.is_empty() {
            return None;
        }
        let width = self.config.dialog.width;
        let position = pos2(
            center.x + self.config.dialog.offset_x - width,
            center.y + self.config.dialog.offset_y,
        );
        let shown = if self.full_text {
            self.text.clone()
        } else {
            self.text
                .chars()
                .take((self.typed_at.elapsed().as_secs_f64() / 0.05) as usize + 1)
                .collect()
        };
        let result = egui::Area::new("dialog".into())
            .constrain(false)
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                let galley = egui::WidgetText::from(
                    egui::RichText::new(&self.text)
                        .font(visual::font(14.))
                        .line_height(Some(21.)),
                )
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Wrap),
                    width - 28.,
                    visual::font(14.),
                );
                let height = (galley.size().y + 20.).min(self.config.dialog.max_height);
                let rect = Rect::from_min_size(ui.min_rect().min, vec2(width, height));
                draw_bubble(ui, rect, &mut self.dialog_bg, false);
                ui.set_min_size(rect.size());
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(rect.shrink2(vec2(14., 10.))),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .max_height((height - 20.).max(1.))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(shown)
                                            .font(visual::font(14.))
                                            .line_height(Some(21.))
                                            .color(Color32::from_gray(51)),
                                    )
                                    .wrap(),
                                );
                            });
                    },
                );
                rect
            });
        Some(result.inner)
    }
    fn input_ui(
        &mut self,
        ctx: &egui::Context,
        screen: Rect,
        center: egui::Pos2,
        image_height: f32,
    ) {
        let width = self.config.dialog.width;
        let error_height = ctx.fonts(|fonts| {
            let mut job = egui::text::LayoutJob::simple(
                self.error.clone(),
                visual::font(13.),
                Color32::from_rgb(231, 76, 60),
                width - 28.,
            );
            job.sections[0].format.line_height = Some(19.5);
            fonts.layout_job(job).size().y + 20.
        });
        let height = 50_f32.max(error_height);
        // Preserve the old 450px component's local boundary checks before translation.
        let origin = center - vec2(225., 225.);
        let position = origin
            + vec2(
                (225. - width / 2.).clamp(10., (screen.width() - 10. - width).max(10.)),
                (225. + image_height / 2. + self.config.dialog.offset_y).clamp(
                    10.,
                    (screen.height() - 10. - self.config.dialog.max_height).max(10.),
                ),
            );
        let rect = Rect::from_min_size(position, vec2(width, height));
        let id = egui::Id::new("message-input");
        let input_transform = ctx
            .layer_transform_to_global(egui::LayerId::new(egui::Order::Foreground, "input".into()))
            .unwrap_or_default();
        let hovered = ctx.input(|i| {
            i.pointer
                .hover_pos()
                .is_some_and(|p| (input_transform * rect).contains(p))
        });
        self.input_bounds = Some(input_transform * rect);
        let focused = ctx.memory(|m| m.has_focus(id));
        let alpha = ctx.animate_value_with_time(
            egui::Id::new("input-alpha"),
            if hovered || focused || self.response.is_some() {
                1.
            } else {
                0.
            },
            0.3,
        );
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
                        if self.response.is_some() {
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
                                    egui::RichText::new("正在思考...")
                                        .font(visual::font(13.))
                                        .color(Color32::from_gray(102)),
                                );
                            });
                        } else if !self.error.is_empty() {
                            ui.add_sized(
                                vec2(width - 28., height - 20.),
                                egui::Label::new(
                                    egui::RichText::new(&self.error)
                                        .font(visual::font(13.))
                                        .line_height(Some(19.5))
                                        .color(Color32::from_rgb(231, 76, 60)),
                                )
                                .wrap(),
                            );
                        } else {
                            let edit = ui.add_sized(
                                vec2(width - 28., 30.),
                                egui::TextEdit::singleline(&mut self.input)
                                    .id(id)
                                    .font(visual::font(14.))
                                    .text_color(Color32::from_gray(51))
                                    .hint_text(
                                        egui::RichText::new("输入消息...")
                                            .color(Color32::from_gray(153)),
                                    )
                                    .margin(egui::Margin::ZERO)
                                    .vertical_align(egui::Align::Center)
                                    .frame(false),
                            );
                            if (edit.has_focus() || edit.lost_focus())
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                self.submit(ctx);
                            }
                        }
                    },
                );
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.response.is_none() {
            self.input.clear();
            self.error.clear();
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
    }
}
impl eframe::App for Pet {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.; 4]
    }
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("window", String::new());
        storage.set_string("sidebarAlwaysVisible", self.pinned.to_string());
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.ui(ctx);
    }
}
impl Pet {
    fn ui(&mut self, ctx: &egui::Context) {
        ctx.request_repaint_after(Duration::from_millis(16));
        #[cfg(target_os = "linux")]
        if let Some(window) = &mut self.drag_window {
            self.drag_sent |= window.update();
        }
        if !self.raised && self.started.elapsed() > Duration::from_millis(300) {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
            self.raised = true;
        }
        if !self.hints_applied && self.started.elapsed() > Duration::from_millis(600) {
            #[cfg(target_os = "linux")]
            if let Some(window) = &self.drag_window {
                window.configure();
            }
            self.hints_applied = true;
        }
        if !self.placed
            && let Some(size) = ctx.input(|i| i.viewport().monitor_size)
        {
            let dev = std::env::args().any(|a| a == "--dev");
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(if dev {
                vec2(800., 600.)
            } else {
                vec2(500., 500.)
            }));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(if dev {
                pos2(100., 100.)
            } else {
                pos2((size.x - 520.).max(0.), 50.)
            }));
            self.placed = true;
        }
        if self.capture.is_some()
            && !self.capture_requested
            && self.started.elapsed() > self.capture_delay
        {
            self.capture_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let screenshot = ctx.input(|i| {
            i.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let (Some(path), Some(image)) = (&self.capture, screenshot) {
            let bytes: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|p| p.to_srgba_unmultiplied())
                .collect();
            if let Err(error) = image::save_buffer(
                path,
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            ) {
                eprintln!("截图失败: {error}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|i| i.pointer.delta() != Vec2::ZERO) {
            self.last_activity = Instant::now();
        }
        if !self.text.is_empty()
            && self.last_activity.elapsed().as_secs_f64() >= self.config.dialog.auto_close_delay
        {
            self.text.clear();
        }
        if self.clock_started.is_some_and(|t| {
            t.elapsed().as_secs_f64() >= self.config.clock_in.work_reminder_hours * 3600.
        }) {
            self.clock_started = None;
            self.say(self.config.clock_in.clock_out_message.clone());
        }
        if let Some(result) = self.response.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.response = None;
            match result {
                Ok(text) => {
                    self.input.clear();
                    self.say(text);
                }
                Err(error) => self.error = error,
            }
        }
        let screen = ctx.screen_rect();
        let time = ctx.input(|i| i.time);
        let base = screen.center();
        let hover = ctx.input(|i| {
            i.pointer.hover_pos().is_some_and(|p| {
                Rect::from_center_size(base, vec2(450., 450.)).contains(p)
                    || self.input_bounds.is_some_and(|r| r.contains(p))
                    || self.dialog_bounds.is_some_and(|r| r.contains(p))
            })
        });
        let scale = ctx.animate_value_with_time(
            egui::Id::new("pet-scale"),
            if hover { 1.05 } else { 1. },
            0.3,
        );
        let lift = ctx.animate_value_with_time(
            egui::Id::new("pet-lift"),
            if hover { -5. } else { 0. },
            0.3,
        );
        let transform = visual::transform(base, (time * 2.).sin() as f32 * 5. + lift, scale);
        for (order, id) in [
            (egui::Order::Background, "pet"),
            (egui::Order::Foreground, "dialog"),
            (egui::Order::Foreground, "input"),
        ] {
            ctx.set_transform_layer(egui::LayerId::new(order, id.into()), transform);
        }
        let center = base;
        let size = self
            .picture
            .as_ref()
            .map(|p| {
                let s = p.size();
                s * (450. / s.x.max(s.y)).min(1.)
            })
            .unwrap_or(vec2(120., 120.));
        let pet_rect = Rect::from_center_size(center, size);
        let mut clicked = false;
        egui::Area::new("pet".into())
            .fixed_pos(screen.min)
            .order(egui::Order::Background)
            .show(ctx, |ui| {
                ui.set_min_size(screen.size());
                if let Some(pic) = self.picture.as_mut() {
                    ui.painter().image(
                        pic.texture().id(),
                        pet_rect,
                        Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                        Color32::WHITE,
                    );
                } else {
                    visual::fallback(ui.painter(), center, time);
                }
                let shadow_phase = (1. - (time * std::f64::consts::PI).cos()) as f32 / 2.;
                ui.painter().add(egui::Shape::ellipse_filled(
                    pos2(base.x, base.y + 225.),
                    vec2(40. + shadow_phase * 10., 10.),
                    Color32::from_black_alpha((7.65 - shadow_phase * 2.55).round() as u8),
                ));
                let menu = ui.interact(
                    Rect::from_center_size(center, vec2(450., 450.)),
                    "context-menu".into(),
                    Sense::click(),
                );
                self.context_menu(ctx, &menu);
                // Match the original circular hit region (radius = displayed image height * 0.3).
                let radius = size.y * 0.3;
                let hit_rect = Rect::from_center_size(center, vec2(radius * 2., radius * 2.));
                let inside = ui.input(|i| {
                    i.pointer
                        .interact_pos()
                        .is_some_and(|p| (transform.inverse() * p).distance(center) <= radius)
                });
                let response =
                    ui.interact(hit_rect, egui::Id::new("pet-drag"), Sense::click_and_drag());
                if inside {
                    ctx.set_cursor_icon(egui::CursorIcon::Move);
                }
                if ctx.input(|i| i.pointer.primary_pressed()) {
                    self.pet_pressed = inside.then(Instant::now);
                    self.drag_sent = false;
                    #[cfg(target_os = "linux")]
                    if inside
                        && response.is_pointer_button_down_on()
                        && let Some(window) = &mut self.drag_window
                        && let Some(offset) = ctx.input(|i| i.pointer.press_origin())
                    {
                        window.arm(offset, ctx.pixels_per_point());
                    }
                }
                let moved = ctx.input(|i| {
                    i.pointer
                        .press_origin()
                        .zip(i.pointer.interact_pos())
                        .is_some_and(|(start, end)| {
                            (transform.inverse() * start).distance(center) <= radius
                                && start.distance(end) > 5.
                        })
                });
                if response.dragged_by(egui::PointerButton::Primary) && moved && !self.drag_sent {
                    #[cfg(target_os = "linux")]
                    if self.drag_window.is_none() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    #[cfg(not(target_os = "linux"))]
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    self.drag_sent = true;
                }
                clicked = response.clicked()
                    && inside
                    && !self.drag_sent
                    && self
                        .pet_pressed
                        .is_some_and(|at| at.elapsed() < Duration::from_millis(300));
                self.context_menu(ctx, &response);
            });
        if clicked {
            self.click_pet();
        }
        let bubble = self.bubble(ctx, center).map(|rect| transform * rect);
        self.dialog_bounds = bubble;
        self.input_ui(ctx, screen, center, size.y);
        let sidebar_handled = self.sidebar(ctx, screen);
        if !clicked
            && !sidebar_handled
            && ctx.input(|i| i.pointer.primary_clicked())
            && let Some(rect) = bubble
        {
            let outside = ctx.input(|i| {
                i.pointer.interact_pos().is_some_and(|p| {
                    !rect.contains(p) && (transform.inverse() * p).distance(center) > size.y * 0.3
                })
            });
            if outside {
                self.text.clear();
            }
        }
        egui::Window::new("关于")
            .open(&mut self.about)
            .show(ctx, |ui| {
                ui.label("Lexington 2.0\n纯 Rust 桌面宠物\neframe / egui");
            });
        egui::Window::new("设置")
            .open(&mut self.settings)
            .show(ctx, |ui| {
                ui.label("配置文件：");
                ui.label(self.root.join("config/config.json").display().to_string());
                ui.label("修改后点击侧边栏 ↻ 重新加载。");
            });
    }
}
fn draw_bubble(ui: &mut egui::Ui, rect: Rect, background: &mut Option<Picture>, input: bool) {
    draw_shadow(ui.painter(), rect, 12., 10., 38);
    if let Some(image) = background {
        let size = image.size();
        let target = rect.size();
        let ratio = (target.x / size.x).max(target.y / size.y);
        let uv_size = target / (size * ratio);
        let uv = Rect::from_center_size(pos2(0.5, 0.5), uv_size);
        egui::Image::new((image.texture().id(), size))
            .uv(uv)
            .corner_radius(12)
            .paint_at(ui, rect);
    } else {
        ui.painter()
            .rect_filled(rect, 12, Color32::from_white_alpha(242));
    }
    ui.painter().add(egui::Shape::convex_polygon(
        visual::bubble_tail(rect, input),
        Color32::from_white_alpha(242),
        Stroke::NONE,
    ));
}
fn draw_shadow(painter: &egui::Painter, rect: Rect, radius: f32, blur: f32, alpha: u8) {
    painter.add(
        egui::epaint::Shadow {
            offset: [0, 2],
            blur: (blur * 2.) as u8,
            spread: 0,
            color: Color32::from_black_alpha(alpha),
        }
        .as_shape(rect, radius as u8),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(app: &mut Pet, ctx: &egui::Context, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(800., 600.))),
                events,
                ..Default::default()
            },
            |ctx| app.ui(ctx),
        );
    }
    fn click(app: &mut Pet, ctx: &egui::Context, point: egui::Pos2) {
        frame(app, ctx, vec![egui::Event::PointerMoved(point)]);
        frame(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
        );
        frame(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }
    #[test]
    fn real_resources_select_cycle_refresh_and_change_format() {
        let ctx = egui::Context::default();
        let mut app = Pet::from_context(&ctx, true);
        assert_eq!(app.illustrations.len(), 30);
        assert_eq!(app.character, "lexington");
        assert_eq!(app.format, "gif");
        assert!(app.picture.is_some());
        assert!(!app.texts.is_empty());
        let old = app.illustrations[app.selected.unwrap()].path.clone();
        app.refresh(&ctx);
        assert_eq!(app.illustrations[app.selected.unwrap()].path, old);
        app.format = "png".into();
        app.select_first(&ctx);
        assert_eq!(app.illustrations[app.selected.unwrap()].format, "png");
        assert!(app.picture.is_some());
    }
    #[test]
    fn native_pointer_events_drive_sidebar_and_pet_click_cycle() {
        let ctx = egui::Context::default();
        let mut app = Pet::from_context(&ctx, true);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, pos2(110., 175.));
        assert!(!app.pinned);
        click(&mut app, &ctx, pos2(110., 325.));
        assert_eq!(app.format, "png");
        click(&mut app, &ctx, pos2(110., 425.));
        assert!(app.clock_started.is_some());
        assert_eq!(app.text, "打卡成功！");
        app.text.clear();
        click(&mut app, &ctx, pos2(400., 300.));
        assert!(!app.text.is_empty(), "first pet click must show a line");
        click(&mut app, &ctx, pos2(400., 300.));
        assert!(app.full_text);
        click(&mut app, &ctx, pos2(400., 300.));
        assert!(app.text.is_empty());
    }
    #[test]
    fn clock_cancel_expiry_and_dialog_inactivity() {
        let ctx = egui::Context::default();
        let mut app = Pet::from_context(&ctx, true);
        app.clock();
        app.clock();
        assert!(app.clock_started.is_none());
        assert_eq!(app.text, "打卡已终止");
        app.config.clock_in.work_reminder_hours = 0.001;
        app.clock_started = Some(Instant::now() - Duration::from_secs(4));
        frame(&mut app, &ctx, vec![]);
        assert!(app.clock_started.is_none());
        assert_eq!(app.text, "下班时间到了！");
        app.last_activity = Instant::now() - Duration::from_secs(11);
        frame(&mut app, &ctx, vec![]);
        assert!(app.text.is_empty());
    }
    #[test]
    fn right_click_in_drag_circle_opens_context_menu() {
        let ctx = egui::Context::default();
        let mut app = Pet::from_context(&ctx, true);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let point = pos2(400., 300.);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(point)]);
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Default::default(),
                }],
            );
        }
        assert!(
            ctx.read_response(egui::Id::new("pet-drag"))
                .is_some_and(|r| r.context_menu_opened())
        );
        assert!(app.text.is_empty());
    }
    #[test]
    fn input_focus_enter_error_and_escape_use_real_ui_events() {
        let ctx = egui::Context::default();
        let mut app = Pet::from_context(&ctx, true);
        // This test exercises the missing-key path without using a developer's API.
        app.root =
            std::env::temp_dir().join(format!("lexington-no-config-{}", rand::random::<u64>()));
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        assert!(!ctx.memory(|m| m.has_focus("message-input".into())));
        click(&mut app, &ctx, pos2(400., 560.));
        assert!(ctx.memory(|m| m.has_focus("message-input".into())));
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Text("测试消息".into()), key(egui::Key::Enter)],
        );
        assert_eq!(app.input, "测试消息");
        assert!(app.response.is_some());
        for _ in 0..100 {
            frame(&mut app, &ctx, vec![]);
            if app.response.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.error.contains("配置不完整"));
        frame(&mut app, &ctx, vec![key(egui::Key::Escape)]);
        assert!(app.input.is_empty());
        assert!(app.error.is_empty());
    }
    #[test]
    fn sidebar_cancel_keeps_new_message_when_dialog_already_open() {
        let ctx = egui::Context::default();
        let mut app = Pet::from_context(&ctx, true);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, pos2(110., 425.));
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.text, "打卡成功！");
        click(&mut app, &ctx, pos2(110., 425.));
        assert_eq!(app.text, "打卡已终止");
        assert!(app.clock_started.is_none());
        click(&mut app, &ctx, pos2(750., 30.));
        assert!(app.text.is_empty());
    }
    #[test]
    fn enter_delivers_successful_llm_reply_through_the_native_ui() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = vec![];
            loop {
                let mut buffer = [0; 4096];
                let n = socket.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length: usize = header
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length: ")
                                .map(str::to_owned)
                        })
                        .unwrap()
                        .parse()
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let body = r#"{"choices":[{"message":{"content":"收到，提督！"}}]}"#;
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        });
        let fixture = std::env::temp_dir().join(format!("lexington-ui-{}", rand::random::<u64>()));
        std::fs::create_dir_all(fixture.join("config")).unwrap();
        std::fs::write(fixture.join("config/config.json"),serde_json::json!({"llm":{"apiUrl":format!("http://{address}/chat"),"apiKey":"local-test","model":"local"}}).to_string()).unwrap();
        let ctx = egui::Context::default();
        let mut app = Pet::from_context(&ctx, true);
        app.root = fixture.clone();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, pos2(400., 560.));
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::Text("你好".into()),
                egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                },
            ],
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.response.is_some() && Instant::now() < deadline {
            frame(&mut app, &ctx, vec![]);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.response.is_none());
        assert!(app.error.is_empty());
        assert!(app.input.is_empty());
        assert_eq!(app.text, "收到，提督！");
        server.join().unwrap();
        std::fs::remove_dir_all(fixture).unwrap();
    }
    #[test]
    fn empty_library_and_character_format_fallback_work_through_buttons() {
        let fixture =
            std::env::temp_dir().join(format!("lexington-characters-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&fixture).unwrap();
        let ctx = egui::Context::default();
        let mut app = Pet::from_root(&ctx, true, fixture.clone());
        assert!(app.picture.is_none());
        assert!(app.selected.is_none());
        assert_eq!(app.format, "gif");
        for (name, folder) in [("a", "png"), ("b", "gif")] {
            let dir = fixture
                .join("resource/illustration")
                .join(name)
                .join(folder);
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join(format!("sample.{folder}"));
            image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
                .save(path)
                .unwrap();
            let texts = fixture.join("resource/texts").join(name);
            std::fs::create_dir_all(&texts).unwrap();
            std::fs::write(texts.join("text.json"), format!(r#"["{name}的台词"]"#)).unwrap();
        }
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, pos2(110., 225.));
        assert_eq!(app.character, "a");
        assert_eq!(app.format, "png");
        assert_eq!(app.texts, ["a的台词"]);
        assert!(app.picture.is_some());
        click(&mut app, &ctx, pos2(110., 375.));
        assert_eq!(app.character, "b");
        assert_eq!(app.format, "gif");
        assert_eq!(app.texts, ["b的台词"]);
        assert!(app.picture.is_some());
        click(&mut app, &ctx, pos2(110., 325.));
        assert_eq!(app.format, "gif"); // b only has GIF, so switching to PNG falls back.
        // Refresh must recover when the selected character disappears, and reload
        // edited text/config/background files instead of retaining stale handles.
        std::fs::remove_file(fixture.join("resource/illustration/b/gif/sample.gif")).unwrap();
        std::fs::write(
            fixture.join("resource/texts/a/text.json"),
            r#"["更新后的台词"]"#,
        )
        .unwrap();
        std::fs::create_dir_all(fixture.join("config")).unwrap();
        std::fs::write(
            fixture.join("config/config.json"),
            r#"{"dialog":{"width":240}}"#,
        )
        .unwrap();
        let backgrounds = fixture.join("resource/background/dialog");
        std::fs::create_dir_all(&backgrounds).unwrap();
        let background = backgrounds.join("custom.jpg");
        image::RgbImage::from_pixel(4, 3, image::Rgb([255, 255, 255]))
            .save(&background)
            .unwrap();
        click(&mut app, &ctx, pos2(110., 225.));
        assert_eq!(app.character, "a");
        assert_eq!(app.format, "png");
        assert_eq!(app.texts, ["更新后的台词"]);
        assert_eq!(app.config.dialog.width, 240.);
        assert_eq!(app.dialog_bg.as_ref().unwrap().size(), vec2(4., 3.));
        std::fs::remove_file(background).unwrap();
        std::fs::remove_file(fixture.join("resource/illustration/a/png/sample.png")).unwrap();
        click(&mut app, &ctx, pos2(110., 225.));
        assert!(app.dialog_bg.is_none());
        assert!(app.picture.is_none());
        assert!(app.selected.is_none());
        assert!(app.character.is_empty());
        assert!(app.texts.is_empty());
        std::fs::remove_dir_all(fixture).unwrap();
    }
    #[test]
    fn browser_sized_cjk_wraps_and_reserves_full_typewriter_height() {
        let ctx = egui::Context::default();
        let mut app = Pet::from_context(&ctx, true);
        app.say("一二三四五六七八九十一二三".into());
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let glyph_width = ctx.fonts(|fonts| {
            fonts
                .layout_no_wrap("一".into(), visual::font(14.), Color32::BLACK)
                .size()
                .x
        });
        assert!(
            (glyph_width - 14.).abs() < 0.5,
            "CJK advance: {glyph_width}"
        );
        let rect = app.dialog_bounds.unwrap();
        assert!(
            (rect.height() - 62.).abs() < 0.1,
            "13 CJK glyphs must occupy two 21px lines, with 20px padding: {rect:?}"
        );
        app.full_text = true;
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.dialog_bounds.unwrap().height(), rect.height());
    }
}
