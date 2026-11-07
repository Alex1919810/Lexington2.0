use super::app::Pet;
use crate::backend::{
    clock,
    config::Config,
    weather::{City, Client, Event, Job},
};
use eframe::egui;

#[derive(Default)]
pub(super) struct WeatherUi {
    client: Option<Client>,
    next_id: u64,
    query: String,
    search: Option<u64>,
    cities: Vec<City>,
    status: String,
    pending: Option<Pending>,
}
struct Pending {
    id: u64,
    city: City,
    started: Option<i64>,
    day: String,
    message: String,
    requested: bool,
    result: Option<Result<String, String>>,
}
pub(super) enum WeatherAction {
    Search(String),
    Select(Option<City>),
    Retry,
}
impl Pet {
    pub(super) fn queue_clock_weather(&mut self, message: String) {
        let Some(city) = self.config.weather.city.clone() else {
            return;
        };
        let Some(started) = self.clock_state.started() else {
            return;
        };
        let Some(time) = clock::local_time(clock::now()) else {
            return;
        };
        self.weather.next_id += 1;
        self.weather.pending = Some(Pending {
            id: self.weather.next_id,
            city,
            started: Some(started),
            day: time.day,
            message,
            requested: false,
            result: None,
        });
    }
    pub(super) fn poll_weather(&mut self, ctx: &egui::Context) {
        if self.weather.pending.as_ref().is_some_and(|p| {
            self.config.weather.city.as_ref() != Some(&p.city)
                || (p.started.is_some()
                    && (self.clock_state.started() != p.started || !self.clock_state.running()))
                || clock::local_time(clock::now()).is_none_or(|t| t.day != p.day)
        }) {
            self.weather.pending = None;
        }
        if self.weather.pending.as_ref().is_some_and(|p| !p.requested) {
            self.weather.status = "正在获取天气…".into();
            if self.weather.client.is_none() {
                let wake = ctx.clone();
                self.weather.client = Some(Client::start(move || wake.request_repaint()));
            }
            let pending = self.weather.pending.as_mut().unwrap();
            pending.requested = true;
            if let Err(error) = self.weather.client.as_ref().unwrap().send(Job::Forecast(
                pending.id,
                pending.city.clone(),
                pending.day.clone(),
            )) {
                pending.result = Some(Err(error));
            }
        }
        for event in self
            .weather
            .client
            .as_ref()
            .map(|c| c.poll())
            .unwrap_or_default()
        {
            match event {
                Event::Cities(id, result) if self.weather.search == Some(id) => {
                    self.weather.search = None;
                    match result {
                        Ok(cities) => {
                            self.weather.status = if cities.is_empty() {
                                "未找到城市，请尝试完整名称或拼音。".into()
                            } else {
                                "请选择城市后保存。".into()
                            };
                            self.weather.cities = cities;
                        }
                        Err(error) => self.weather.status = error,
                    }
                }
                Event::Forecast(id, result) => {
                    if let Some(pending) = &mut self.weather.pending
                        && pending.id == id
                    {
                        self.weather.status = match &result {
                            Ok(_) => "天气已更新，将在对话框显示。".into(),
                            Err(error) => error.clone(),
                        };
                        pending.result = Some(result);
                    }
                }
                _ => {}
            }
        }
        // Let a chat/alarm finish before displaying a delayed weather result.
        if !self.busy
            && self.weather.pending.as_ref().is_some_and(|p| {
                p.result.is_some() && (self.text.is_empty() || self.text == p.message)
            })
        {
            let pending = self.weather.pending.take().unwrap();
            let text = match pending.result.unwrap() {
                Ok(text) => text,
                Err(error) => format!("天气暂时不可用：{error}\n可在“设置 → 天气”重试。"),
            };
            self.say(format!("{}\n{text}", pending.message));
            self.full_text = true;
        }
    }
    pub(super) fn weather_settings_ui(&mut self, ui: &mut egui::Ui) -> Option<WeatherAction> {
        let mut action = None;
        egui::Frame::group(ui.style())
            .fill(egui::Color32::WHITE)
            .corner_radius(8)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.strong("打卡天气");
                ui.label("每天打卡后，在对话框显示三天天气。");
                if let Some(city) = &self.config.weather.city {
                    ui.label(format!("当前：{}", city.name));
                    ui.horizontal(|ui| {
                        if ui.button("查看天气").clicked() {
                            action = Some(WeatherAction::Retry);
                        }
                        if ui.button("停用").clicked() {
                            action = Some(WeatherAction::Select(None));
                        }
                    });
                } else {
                    ui.weak("尚未设置城市");
                }
                ui.separator();
                ui.label("城市");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.weather.query)
                            .id(egui::Id::new("weather-city-query"))
                            .desired_width(f32::INFINITY)
                            .char_limit(100)
                            .hint_text("城市名称，如：上海 / Shanghai"),
                    )
                    .changed()
                {
                    self.weather.search = None;
                    self.weather.cities.clear();
                    self.weather.status.clear();
                }
                if ui
                    .add_enabled(
                        self.weather.search.is_none()
                            && self.weather.query.trim().chars().count() >= 2,
                        egui::Button::new(if self.weather.search.is_some() {
                            "搜索中…"
                        } else {
                            "搜索城市"
                        }),
                    )
                    .clicked()
                {
                    action = Some(WeatherAction::Search(self.weather.query.trim().into()));
                }
                for city in &self.weather.cities {
                    if ui.button(format!("选择并保存：{}", city.name)).clicked() {
                        action = Some(WeatherAction::Select(Some(city.clone())));
                    }
                }
                if !self.weather.status.is_empty() {
                    ui.label(&self.weather.status);
                }
                ui.hyperlink_to("天气数据：Open-Meteo", "https://open-meteo.com/");
            });
        action
    }
    pub(super) fn weather_action(&mut self, action: WeatherAction, ctx: &egui::Context) {
        match action {
            WeatherAction::Search(query) => {
                if query.chars().count() < 2 || query.len() > 400 {
                    return;
                }
                if self.weather.client.is_none() {
                    let wake = ctx.clone();
                    self.weather.client = Some(Client::start(move || wake.request_repaint()));
                }
                self.weather.next_id += 1;
                let id = self.weather.next_id;
                match self
                    .weather
                    .client
                    .as_ref()
                    .unwrap()
                    .send(Job::Search(id, query))
                {
                    Ok(()) => {
                        self.weather.search = Some(id);
                        self.weather.status.clear();
                    }
                    Err(error) => self.weather.status = error,
                }
            }
            WeatherAction::Select(city) => match Config::save_weather(&self.root, city.as_ref()) {
                Ok(()) => {
                    self.config.weather.city = city;
                    self.weather.pending = None;
                    self.weather.search = None;
                    self.weather.cities.clear();
                    self.weather.status = "已保存，下次打卡生效。".into();
                }
                Err(error) => self.weather.status = error,
            },
            WeatherAction::Retry => {
                // Explicit preview is also available before today's clock-in.
                if let Some(city) = self.config.weather.city.clone()
                    && let Some(time) = clock::local_time(clock::now())
                {
                    if !self.busy {
                        self.say("天气预报".into());
                    }
                    self.weather.next_id += 1;
                    let started = None;
                    self.weather.pending = Some(Pending {
                        id: self.weather.next_id,
                        city,
                        started,
                        day: time.day,
                        message: "天气预报".into(),
                        requested: false,
                        result: None,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::activity::{Monitor, Sample};
    use std::time::{Duration, Instant};

    fn city() -> City {
        City {
            name: "上海 · 中国".into(),
            latitude: 31.23,
            longitude: 121.47,
            timezone: "Asia/Shanghai".into(),
        }
    }
    fn app() -> (Pet, egui::Context, std::path::PathBuf) {
        let root =
            std::env::temp_dir().join(format!("lexington-weather-ui-{}", rand::random::<u64>()));
        Config::save_weather(&root, None).unwrap();
        let ctx = egui::Context::default();
        (Pet::from_root(&ctx, false, root.clone()), ctx, root)
    }
    fn frame(app: &mut Pet, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.ui(ctx),
        )
    }
    #[test]
    fn weather_city_selection_uses_settings_ui_and_persists_without_starting_requests() {
        let (mut app, ctx, root) = app();
        app.settings = true;
        app.settings_page = 4;
        app.weather.query = "上海".into();
        app.weather.cities = vec![city()];
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        let output = frame(&mut app, &ctx, vec![]);
        let mut shapes: Vec<_> = output.shapes.iter().map(|s| &s.shape).collect();
        let mut point = None;
        while let Some(shape) = shapes.pop() {
            match shape {
                egui::Shape::Vec(children) => shapes.extend(children),
                egui::Shape::Text(text) if text.galley.job.text.starts_with("选择并保存：") =>
                {
                    point = Some(text.pos + text.galley.size() / 2.);
                    break;
                }
                _ => {}
            }
        }
        let point = point.expect("weather city selection button");
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(point)]);
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                }],
            );
        }
        assert_eq!(app.config.weather.city, Some(city()));
        assert_eq!(Config::read(&root).unwrap().weather.city, Some(city()));
        assert!(
            app.weather.client.is_none(),
            "selecting a city must not fetch forecasts"
        );
        frame(&mut app, &ctx, vec![]);
        assert!(
            app.weather.client.is_none(),
            "idle frames must not fetch weather"
        );
        app.weather_action(WeatherAction::Select(None), &ctx);
        assert!(Config::read(&root).unwrap().weather.city.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn weather_clock_result_waits_for_chat_and_is_discarded_after_cancel_or_city_change() {
        let (mut app, ctx, root) = app();
        app.weather_action(WeatherAction::Select(Some(city())), &ctx);
        app.clock();
        let pending = app
            .weather
            .pending
            .as_mut()
            .expect("manual clock-in must queue weather");
        pending.requested = true;
        pending.result = Some(Ok("今天 多云 19～25°C".into()));
        app.busy = true;
        app.text = "正在回复".into();
        app.poll_weather(&ctx);
        assert_eq!(app.text, "正在回复");
        app.busy = false;
        app.text = "聊天完成".into();
        app.poll_weather(&ctx);
        assert_eq!(app.text, "聊天完成");
        app.text.clear();
        app.poll_weather(&ctx);
        assert!(app.text.contains("打卡成功！\n今天 多云"));
        assert!(app.full_text);
        assert!(app.weather.pending.is_none());
        assert!(app.weather.client.is_none());
        app.clock(); // cancel
        assert!(app.weather.pending.is_none());
        app.clock(); // start again, but cancel before its response
        app.weather.pending.as_mut().unwrap().requested = true;
        app.weather.pending.as_mut().unwrap().result = Some(Ok("旧天气".into()));
        app.clock();
        app.poll_weather(&ctx);
        assert!(app.weather.pending.is_none());
        assert_eq!(app.text, "打卡已终止");
        app.clock();
        app.config.weather.city.as_mut().unwrap().longitude += 1.;
        app.poll_weather(&ctx);
        assert!(app.weather.pending.is_none());
        assert!(app.weather.client.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn weather_automatic_clock_queues_forecast_and_preview_does_not_require_clock_in() {
        let (mut app, ctx, root) = app();
        app.weather_action(WeatherAction::Select(Some(city())), &ctx);
        let now = clock::now();
        let tick = Instant::now();
        for seconds in (0..180).step_by(2) {
            assert!(
                !app.clock_state
                    .observe(
                        &app.config.clock_in,
                        now - 180 + seconds,
                        tick - Duration::from_secs((180 - seconds) as u64),
                        Some(Duration::ZERO)
                    )
                    .unwrap()
            );
        }
        app.activity_monitor = Some(Monitor::mock(Sample {
            idle: Some(Duration::ZERO),
            status: "active".into(),
            sampled: tick,
        }));
        app.poll_clock();
        assert!(
            app.weather
                .pending
                .as_ref()
                .unwrap()
                .message
                .contains("开始工作")
        );
        assert!(app.clock_state.running());
        app.weather.pending.as_mut().unwrap().requested = true;
        app.weather.pending.as_mut().unwrap().result = Some(Err("模拟超时".into()));
        app.poll_weather(&ctx);
        assert!(app.text.contains("天气暂时不可用"));
        assert!(
            app.clock_state.running(),
            "weather failure must not undo clock-in"
        );
        app.clock();
        app.weather_action(WeatherAction::Retry, &ctx);
        let pending = app.weather.pending.as_mut().unwrap();
        assert!(pending.started.is_none());
        pending.requested = true;
        pending.result = Some(Ok("预览天气".into()));
        app.text.clear();
        app.poll_weather(&ctx);
        assert!(app.text.contains("预览天气"));
        assert!(!app.clock_state.running());
        std::fs::remove_dir_all(root).unwrap();
    }
}
