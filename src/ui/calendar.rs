use super::{app::Wife, style};
use crate::{
    backend::{
        calendar::{CalendarState, Date, Overtime, duration_label, month_festivals, normal_off},
        clock::{self, local_time},
    },
    tools::paths,
};
use eframe::egui;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct CalendarUi {
    pub open: bool,
    state: Option<CalendarState>,
    month: Option<Date>,
    festival_month: Option<(i32, u32)>,
    festivals: BTreeMap<u32, Vec<String>>,
    selected: Option<Date>,
    error: String,
    notice: String,
    action: Option<Action>,
    confirm_delete: bool,
    manual_open: bool,
    draft_day: Option<Date>,
    draft_hours: String,
    draft_minutes: String,
    started: Option<Instant>,
    raised: bool,
}

enum Action {
    Reload,
    CheckIn,
    Manual(Date),
    Delete(Date),
}

impl Wife {
    pub(super) fn open_calendar(&mut self) {
        if self.calendar.state.is_none() {
            self.calendar.action = Some(Action::Reload);
            self.apply_calendar_action(clock::now());
        }
        if self.calendar.month.is_none() {
            self.calendar.month = Date::at(clock::now());
            self.calendar.selected = self.calendar.month;
        }
        self.calendar.open = true;
    }

    pub(super) fn calendar_window(&mut self, ctx: &egui::Context) {
        if !self.calendar.open {
            return;
        }
        let id = egui::ViewportId::from_hash_of("lexington-calendar");
        let size = egui::vec2(560., 620.);
        let mut builder = egui::ViewportBuilder::default()
            .with_title("日历 · 加班统计")
            .with_inner_size(size)
            .with_min_inner_size(egui::vec2(500., 560.))
            .with_always_on_top();
        if self.calendar.started.is_none() {
            self.calendar.started = Some(Instant::now());
            self.calendar.raised = false;
            let mut monitor = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                ctx.input(|input| input.viewport().monitor_size)
                    .unwrap_or(egui::vec2(1920., 1080.)),
            );
            #[cfg(any(target_os = "linux", windows))]
            if let Some(window) = &self.drag_window {
                let monitors = window.monitors(ctx.pixels_per_point());
                if let Some(found) = self
                    .window_position
                    .and_then(|position| {
                        monitors
                            .iter()
                            .find(|rect| rect.contains(position + ctx.screen_rect().size() / 2.))
                    })
                    .or_else(|| monitors.first())
                {
                    monitor = *found;
                }
            }
            let near = self.window_position.unwrap_or(monitor.min);
            builder = builder.with_position(egui::pos2(
                near.x.clamp(
                    monitor.left() + 8.,
                    (monitor.right() - size.x - 8.).max(monitor.left() + 8.),
                ),
                near.y.clamp(
                    monitor.top() + 8.,
                    (monitor.bottom() - size.y - 48.).max(monitor.top() + 8.),
                ),
            ));
        }
        let mut open = true;
        ctx.show_viewport_immediate(id, builder, |calendar, class| {
            if class == egui::ViewportClass::Embedded {
                if let Some(window) = egui::Window::new("日历 · 加班统计")
                    .open(&mut open)
                    .order(egui::Order::Foreground)
                    .collapsible(false)
                    .default_size(egui::vec2(536., 580.))
                    .show(calendar, |ui| self.calendar_ui(ui, clock::now()))
                    && open
                {
                    self.input_regions.push(window.response.rect);
                }
            } else {
                if !self.calendar.raised
                    && self
                        .calendar
                        .started
                        .is_some_and(|at| at.elapsed() >= Duration::from_millis(800))
                {
                    calendar.send_viewport_cmd(egui::ViewportCommand::Focus);
                    self.calendar.raised = true;
                }
                open = !calendar.input(|input| input.viewport().close_requested());
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(style::PANEL_BG).inner_margin(12))
                    .show(calendar, |ui| self.calendar_ui(ui, clock::now()));
            }
        });
        self.apply_calendar_action(clock::now());
        self.calendar.open = open;
        if !open {
            self.calendar.started = None;
            self.calendar.confirm_delete = false;
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
        }
    }

    fn calendar_ui(&mut self, ui: &mut egui::Ui, now: i64) {
        style::panel_style(ui);
        let today = Date::at(now);
        let off = if self.clock_storage_ok {
            normal_off(
                self.clock_state.started(),
                self.config.clock_in.work_reminder_hours,
                now,
            )
        } else {
            Err("打卡记录读取失败，请先处理打卡设置中的错误。".into())
        };
        let calendar = &mut self.calendar;
        let Some(mut month) = calendar.month.or(today) else {
            ui.label("无法读取本地日期");
            return;
        };
        ui.horizontal(|ui| {
            if ui.button("上个月").clicked() {
                month = month.shift_month(-1);
            }
            ui.heading(format!("{}年{:02}月", month.year, month.month));
            if ui.button("下个月").clicked() {
                month = month.shift_month(1);
            }
            if ui.button("今天").clicked()
                && let Some(today) = today
            {
                month = today;
                calendar.selected = Some(today);
                calendar.confirm_delete = false;
            }
        });
        if calendar
            .selected
            .is_none_or(|old| old.year != month.year || old.month != month.month)
        {
            calendar.confirm_delete = false;
            calendar.selected = Some(Date { day: 1, ..month });
        }
        calendar.month = Some(month);
        if calendar.festival_month != Some((month.year, month.month)) {
            calendar.festivals = month_festivals(month);
            calendar.festival_month = Some((month.year, month.month));
        }
        if !calendar.error.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(174, 54, 54), &calendar.error);
        }
        let Some(state) = &calendar.state else {
            if ui.button("重新读取").clicked() {
                calendar.action = Some(Action::Reload);
            }
            return;
        };
        let (days, minutes) = state.month_total(month);
        style::card().inner_margin(10).show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("本月加班  {}", duration_label(minutes)))
                    .strong()
                    .size(18.),
            );
            ui.weak(format!("共 {days} 天 · 暖色为节日 · 加班时长（小时:分钟）"))
                .on_hover_text("节日仅标注名称，放假及调休以官方安排为准");
            if !(1900..=2100).contains(&month.year) {
                ui.weak("农历节日支持 1900～2100 年，当前仅显示公历节日");
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("calendar-content")
            .auto_shrink([false, false])
            .min_scrolled_height(0.)
            .show(ui, |ui| {
                let width = (ui.available_width() - 6. * 4.) / 7.;
                ui.scope(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(4., 2.);
                    egui::Grid::new("calendar-month")
                        .spacing(egui::vec2(4., 4.))
                        .show(ui, |ui| {
                            for label in ["一", "二", "三", "四", "五", "六", "日"] {
                                ui.add_sized([width, 20.], egui::Label::new(label));
                            }
                            ui.end_row();
                            for cell in 0..42 {
                                let number = cell - month.first_weekday() as i32 + 1;
                                if number < 1 || number > month.days_in_month() as i32 {
                                    ui.allocate_space(egui::vec2(width, 40.));
                                } else {
                                    let date = Date {
                                        day: number as u32,
                                        ..month
                                    };
                                    let festivals = calendar.festivals.get(&date.day);
                                    let mut label = number.to_string();
                                    if let Some(names) = festivals {
                                        label.push('\n');
                                        label.push_str(&names[0]);
                                        if names.len() > 1 {
                                            label.push('+');
                                        }
                                    }
                                    if let Some(entry) = state.entry(date) {
                                        let minutes = entry.minutes();
                                        label.push_str(&format!(
                                            "\n{}:{:02}",
                                            minutes / 60,
                                            minutes % 60
                                        ));
                                    }
                                    let mut text = egui::RichText::new(label).size(12.);
                                    if festivals.is_some() {
                                        text = text.color(egui::Color32::from_rgb(162, 75, 36));
                                    }
                                    let mut button = egui::Button::new(text)
                                        .wrap()
                                        .selected(calendar.selected == Some(date));
                                    if today == Some(date) {
                                        button = button
                                            .stroke(egui::Stroke::new(1.5_f32, style::ACCENT));
                                    }
                                    let response = ui.add_sized([width, 40.], button);
                                    let response = if let Some(names) = festivals {
                                        response.on_hover_text(names.join(" · "))
                                    } else {
                                        response
                                    };
                                    if response.clicked() {
                                        calendar.selected = Some(date);
                                        calendar.confirm_delete = false;
                                    }
                                }
                                if cell % 7 == 6 {
                                    ui.end_row();
                                }
                            }
                        });
                });
                ui.add_space(8.);
                style::card().inner_margin(10).show(ui, |ui| {
                    if calendar.manual_open
                        && let Some(date) = calendar.selected
                    {
                        if calendar.draft_day != Some(date) {
                            let minutes = state.entry(date).map_or(0, |entry| entry.minutes());
                            calendar.draft_hours = (minutes / 60).to_string();
                            calendar.draft_minutes = format!("{:02}", minutes % 60);
                            calendar.draft_day = Some(date);
                        }
                        ui.label(format!("手动填写：{} · 每日 0～24 小时", date.label()));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut calendar.draft_hours)
                                    .id(egui::Id::new("calendar-manual-hours"))
                                    .desired_width(42.)
                                    .char_limit(2),
                            );
                            ui.label("小时");
                            ui.add(
                                egui::TextEdit::singleline(&mut calendar.draft_minutes)
                                    .id(egui::Id::new("calendar-manual-minutes"))
                                    .desired_width(42.)
                                    .char_limit(2),
                            );
                            ui.label("分钟");
                            if ui.button("保存加班时长").clicked() {
                                calendar.action = Some(Action::Manual(date));
                            }
                        });
                    } else {
                        match &off {
                            Ok(time) => {
                                let end = local_time(*time).unwrap();
                                ui.label(format!("预计下班：{} {}", end.day, end.label()));
                            }
                            Err(error) => {
                                ui.label(error);
                            }
                        }
                        ui.weak("下班时间 = 今天上班打卡 + 配置的工作时长");
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                off.as_ref().is_ok_and(|time| now >= *time),
                                egui::Button::new("加班打卡"),
                            )
                            .clicked()
                        {
                            calendar.action = Some(Action::CheckIn);
                        }
                        if ui
                            .selectable_label(calendar.manual_open, "手动填写")
                            .clicked()
                        {
                            calendar.manual_open = !calendar.manual_open;
                        }
                        if ui.button("打卡设置").clicked() {
                            self.settings_page = 2;
                            self.settings = true;
                        }
                    });
                    if !calendar.manual_open && off.as_ref().is_ok_and(|time| now < *time) {
                        ui.weak("还未到下班时间");
                    }
                    if !calendar.notice.is_empty() {
                        ui.label(&calendar.notice);
                    }
                });
                if let Some(date) = calendar.selected {
                    ui.add_space(6.);
                    ui.label(egui::RichText::new(date.label()).strong());
                    if let Some(names) = calendar.festivals.get(&date.day) {
                        ui.colored_label(
                            egui::Color32::from_rgb(162, 75, 36),
                            format!("节日：{}", names.join(" · ")),
                        );
                    }
                    if let Some(entry) = state.entry(date) {
                        ui.label(format!("加班：{}", duration_label(entry.minutes())));
                        match entry {
                            Overtime::Measured { off_at, checked_at } => {
                                for (label, time) in
                                    [("下班时间", off_at), ("加班打卡", checked_at)]
                                {
                                    if let Some(time) = local_time(*time) {
                                        ui.weak(format!("{label}：{} {}", time.day, time.label()));
                                    }
                                }
                            }
                            Overtime::Manual { .. } => {
                                ui.weak("手动填写的加班时长");
                            }
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .button(if calendar.confirm_delete {
                                    "确认删除"
                                } else {
                                    "删除这天记录"
                                })
                                .clicked()
                            {
                                if calendar.confirm_delete {
                                    calendar.action = Some(Action::Delete(date));
                                } else {
                                    calendar.confirm_delete = true;
                                }
                            }
                            if calendar.confirm_delete && ui.button("取消").clicked() {
                                calendar.confirm_delete = false;
                            }
                        });
                    } else {
                        ui.weak("这天没有加班记录");
                    }
                }
            });
    }

    fn apply_calendar_action(&mut self, now: i64) {
        let Some(action) = self.calendar.action.take() else {
            return;
        };
        let result = match action {
            Action::Reload => CalendarState::load(paths::data_root().join(if self.dev {
                "calendar-dev.json"
            } else {
                "calendar.json"
            }))
            .map(|state| self.calendar.state = Some(state)),
            Action::CheckIn => {
                let off = if self.clock_storage_ok {
                    normal_off(
                        self.clock_state.started(),
                        self.config.clock_in.work_reminder_hours,
                        now,
                    )
                } else {
                    Err("打卡记录不可用。".into())
                };
                off.and_then(|off| {
                    let state = self.calendar.state.as_mut().ok_or("加班记录未加载。")?;
                    let day = state.check_in(off, now)?;
                    self.calendar.selected = Some(day);
                    self.calendar.month = Some(day);
                    self.calendar.confirm_delete = false;
                    self.calendar.draft_day = None;
                    self.calendar.notice = format!(
                        "已记录 {} 加班 {}",
                        day.label(),
                        duration_label(state.entry(day).unwrap().minutes())
                    );
                    Ok(())
                })
            }
            Action::Manual(day) => {
                let hours = self.calendar.draft_hours.trim().parse::<u32>();
                let minutes = self.calendar.draft_minutes.trim().parse::<u32>();
                match (hours, minutes) {
                    (Ok(hours), Ok(minutes)) => self
                        .calendar
                        .state
                        .as_mut()
                        .ok_or("加班记录未加载。".to_owned())
                        .and_then(|state| state.set_manual(day, hours, minutes))
                        .map(|()| {
                            self.calendar.confirm_delete = false;
                            self.calendar.draft_day = None;
                            self.calendar.notice = format!(
                                "已保存 {} 加班 {}",
                                day.label(),
                                duration_label(u64::from(hours) * 60 + u64::from(minutes))
                            );
                        }),
                    _ => Err("请填写有效的小时和分钟数字。".into()),
                }
            }
            Action::Delete(day) => self
                .calendar
                .state
                .as_mut()
                .ok_or("加班记录未加载。".to_owned())
                .and_then(|state| state.delete(day))
                .map(|()| {
                    self.calendar.confirm_delete = false;
                    self.calendar.notice.clear();
                    self.calendar.draft_day = None;
                }),
        };
        if result.is_err() {
            self.calendar.notice.clear();
        }
        self.calendar.error = result.err().unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now(hour: i32) -> i64 {
        let mut date = crate::tools::time::local(1_800_000_000).unwrap();
        date.tm_year = 126;
        date.tm_mon = 10;
        date.tm_mday = 12;
        date.tm_hour = hour;
        date.tm_min = 0;
        date.tm_sec = 0;
        date.tm_isdst = -1;
        crate::tools::time::timestamp(&mut date)
    }

    fn frame(
        app: &mut Wife,
        ctx: &egui::Context,
        now: i64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500., 560.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(style::PANEL_BG).inner_margin(12))
                    .show(ctx, |ui| app.calendar_ui(ui, now));
            },
        );
        app.apply_calendar_action(now);
        output
    }

    fn click(app: &mut Wife, ctx: &egui::Context, now: i64, label: &str) {
        let output = frame(app, ctx, now, vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == label
                {
                    let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
                    assert!(
                        shape.clip_rect.contains_rect(rect),
                        "clipped calendar control: {label}"
                    );
                    Some(rect.center())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("missing calendar control: {label}"));
        assert!(
            position.x < 500. && position.y < 560.,
            "calendar control outside minimum viewport: {label}"
        );
        frame(app, ctx, now, vec![egui::Event::PointerMoved(position)]);
        for pressed in [true, false] {
            frame(
                app,
                ctx,
                now,
                vec![egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                }],
            );
        }
    }

    #[test]
    fn calendar_festivals_remain_visible_with_overtime_and_refresh_after_navigation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("calendar.json");
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, directory.path().to_path_buf());
        let day = Date::parse("2020-10-01").unwrap();
        let mut state = CalendarState::load(path.clone()).unwrap();
        state.set_manual(day, 2, 30).unwrap();
        app.calendar.state = Some(state);
        app.calendar.month = Some(day);
        app.calendar.selected = Some(day);
        app.input = "保留草稿".into();
        app.text = "保留回复".into();
        let output = frame(&mut app, &ctx, now(20), vec![]);
        let label = "1\n国庆节+\n2:30";
        assert!(
            output.shapes.iter().any(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == label
                {
                    let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
                    assert!(
                        shape.clip_rect.contains_rect(rect),
                        "clipped festival: {label}"
                    );
                    assert!(!text.galley.elided, "festival or overtime was truncated");
                    true
                } else {
                    false
                }
            }),
            "missing festival label: {label}"
        );
        click(&mut app, &ctx, now(20), "下个月");
        assert_eq!(app.calendar.festival_month, Some((2020, 11)));
        assert!(!app.calendar.festivals.contains_key(&1));
        click(&mut app, &ctx, now(20), "上个月");
        assert_eq!(app.calendar.festivals[&1], ["国庆节", "中秋节"]);
        let mut detail_visible = false;
        for _ in 0..15 {
            let output = frame(
                &mut app,
                &ctx,
                now(20),
                vec![
                    egui::Event::PointerMoved(egui::pos2(450., 450.)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0., -100.),
                        modifiers: Default::default(),
                    },
                ],
            );
            detail_visible = output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text)
                    if text.galley.job.text == "节日：国庆节 · 中秋节"
                    && shape.clip_rect.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size())))
            });
            if detail_visible {
                break;
            }
        }
        assert!(
            detail_visible,
            "full festival names must be reachable by scrolling"
        );
        assert_eq!(
            CalendarState::load(path)
                .unwrap()
                .entry(day)
                .unwrap()
                .minutes(),
            150
        );
        assert_eq!(app.input, "保留草稿");
        assert_eq!(app.text, "保留回复");
        assert!(!app.busy && app.codex.is_none());
    }

    #[test]
    fn calendar_buttons_record_update_navigate_and_keep_chat_isolated() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("calendar.json");
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, directory.path().to_path_buf());
        app.calendar.state = Some(CalendarState::load(path.clone()).unwrap());
        app.calendar.month = Date::at(now(20));
        app.calendar.selected = app.calendar.month;
        app.open_calendar();
        app.config.clock_in.work_reminder_hours = 8.;
        app.clock_state.edit("09:00", now(20)).unwrap();
        app.input = "保留聊天草稿".into();
        app.text = "保留原回复".into();
        app.busy = true;
        for _ in 0..3 {
            frame(&mut app, &ctx, now(20), vec![]);
        }
        click(&mut app, &ctx, now(16), "加班打卡");
        assert!(!path.exists(), "early click must not create a record");
        click(&mut app, &ctx, now(20), "加班打卡");
        let today = Date::at(now(20)).unwrap();
        assert_eq!(
            app.calendar.state.as_ref().unwrap().month_total(today),
            (1, 180)
        );
        click(&mut app, &ctx, now(21), "加班打卡");
        assert_eq!(
            CalendarState::load(path.clone())
                .unwrap()
                .month_total(today),
            (1, 240)
        );
        let output = frame(&mut app, &ctx, now(21), vec![]);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "12\n4:00")));
        click(&mut app, &ctx, now(21), "下个月");
        assert_eq!(app.calendar.month.unwrap().month, 12);
        click(&mut app, &ctx, now(21), "今天");
        assert_eq!(app.calendar.selected, Some(today));
        app.calendar.open = false;
        app.open_calendar();
        assert_eq!(
            app.calendar.state.as_ref().unwrap().month_total(today),
            (1, 240)
        );
        click(&mut app, &ctx, now(21), "打卡设置");
        assert!(app.settings && app.settings_page == 2);
        assert_eq!(app.text, "保留原回复");
        assert_eq!(app.input, "保留聊天草稿");
        assert!(app.busy && app.codex.is_none());
        // Force a failed write after one successfully persisted check-in.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        click(&mut app, &ctx, now(22), "加班打卡");
        assert!(!app.calendar.error.is_empty());
        assert_eq!(
            app.calendar.state.as_ref().unwrap().month_total(today),
            (1, 240)
        );
    }

    #[test]
    fn manual_calendar_entries_edit_history_without_clock_and_preserve_failed_drafts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("calendar.json");
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, directory.path().to_path_buf());
        app.calendar.state = Some(CalendarState::load(path.clone()).unwrap());
        app.calendar.month = Date::at(now(20));
        app.calendar.selected = app.calendar.month;
        app.clock_storage_ok = false;
        app.input = "保留草稿".into();
        app.text = "保留回复".into();
        app.busy = true;
        for _ in 0..3 {
            frame(&mut app, &ctx, now(20), vec![]);
        }
        click(&mut app, &ctx, now(20), "上个月");
        click(&mut app, &ctx, now(20), "手动填写");
        let day = Date::parse("2026-10-01").unwrap();
        assert_eq!(app.calendar.selected, Some(day));
        frame(&mut app, &ctx, now(20), vec![]);
        for (id, value) in [
            ("calendar-manual-hours", "2"),
            ("calendar-manual-minutes", "30"),
        ] {
            let point = ctx.read_response(egui::Id::new(id)).unwrap().rect.center();
            frame(
                &mut app,
                &ctx,
                now(20),
                vec![egui::Event::PointerMoved(point)],
            );
            for pressed in [true, false] {
                frame(
                    &mut app,
                    &ctx,
                    now(20),
                    vec![egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    }],
                );
            }
            frame(
                &mut app,
                &ctx,
                now(20),
                vec![
                    egui::Event::Key {
                        key: egui::Key::A,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers {
                            ctrl: true,
                            command: true,
                            ..Default::default()
                        },
                    },
                    egui::Event::Text(value.into()),
                ],
            );
        }
        click(&mut app, &ctx, now(20), "保存加班时长");
        assert!(app.calendar.error.is_empty(), "{}", app.calendar.error);
        assert_eq!(
            CalendarState::load(path.clone()).unwrap().month_total(day),
            (1, 150)
        );
        click(&mut app, &ctx, now(20), "2");
        assert_eq!(app.calendar.draft_hours, "0");
        click(&mut app, &ctx, now(20), "1\n国庆节\n2:30");
        assert_eq!(app.calendar.draft_hours, "2");
        assert_eq!(app.calendar.draft_minutes, "30");
        app.calendar.draft_hours = "3".into();
        click(&mut app, &ctx, now(20), "保存加班时长");
        assert_eq!(
            app.calendar.state.as_ref().unwrap().month_total(day),
            (1, 210)
        );
        app.calendar.open = false;
        app.open_calendar();
        assert!(app.calendar.manual_open);
        assert_eq!(app.calendar.draft_hours, "3");
        frame(&mut app, &ctx, now(20), vec![]);
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        app.calendar.draft_hours = "4".into();
        click(&mut app, &ctx, now(20), "保存加班时长");
        assert!(!app.calendar.error.is_empty());
        assert_eq!(app.calendar.draft_hours, "4");
        assert_eq!(app.calendar.draft_minutes, "30");
        assert_eq!(
            app.calendar.state.as_ref().unwrap().month_total(day),
            (1, 210)
        );
        assert_eq!(app.input, "保留草稿");
        assert_eq!(app.text, "保留回复");
        assert!(app.busy && app.codex.is_none());
    }

    #[test]
    fn calendar_menu_opens_without_starting_models_or_loading_unrequested_data() {
        let directory = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, directory.path().to_path_buf());
        assert!(app.calendar.state.is_none());
        app.calendar.state =
            Some(CalendarState::load(directory.path().join("calendar.json")).unwrap());
        let mut run = |events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500., 560.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let response =
                            ui.allocate_response(egui::vec2(100., 100.), egui::Sense::click());
                        app.context_menu(ctx, &response);
                    });
                },
            )
        };
        run(vec![]);
        let point = egui::pos2(40., 40.);
        run(vec![egui::Event::PointerMoved(point)]);
        for pressed in [true, false] {
            run(vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        let output = run(vec![]);
        let point = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "日历"
                {
                    Some(text.pos + text.galley.size() / 2.)
                } else {
                    None
                }
            })
            .expect("calendar menu entry");
        run(vec![egui::Event::PointerMoved(point)]);
        for pressed in [true, false] {
            run(vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        assert!(app.calendar.open);
        assert!(!app.settings && !app.memo.open && !app.schedule.open);
        assert!(app.codex.is_none());
    }
}
