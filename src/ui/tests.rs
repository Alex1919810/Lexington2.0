use super::{app::Wife, style};
use eframe::egui::{self, Color32, Rect, pos2, vec2};
use std::time::{Duration, Instant};
fn frame(app: &mut Wife, ctx: &egui::Context, events: Vec<egui::Event>) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(800., 600.))),
            events,
            ..Default::default()
        },
        |ctx| app.ui(ctx),
    );
}
fn click(app: &mut Wife, ctx: &egui::Context, point: egui::Pos2) {
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
fn idle_pet_keeps_a_nonzero_refresh_timer_after_focus_moves_away() {
    let directory = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = Wife::from_root(&ctx, false, directory.path().to_path_buf());
    for focused in [true, false] {
        // Settle initial widget animations before inspecting the idle timer.
        for _ in 0..60 {
            let mut input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(500., 500.))),
                focused,
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .focused = Some(focused);
            let output = ctx.run(input, |ctx| app.ui(ctx));
            if ctx.cumulative_pass_nr() % 60 == 0 {
                let delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
                assert!(
                    delay >= Duration::from_millis(16),
                    "busy repaint loop: {delay:?}"
                );
                assert!(
                    delay <= Duration::from_millis(17),
                    "animation timer stopped: {delay:?}"
                );
            }
        }
    }
}

#[test]
fn character_switch_discards_old_codex_session_and_missing_persona_cannot_reuse_it() {
    let root = std::env::temp_dir().join(format!("lexington-ui-persona-{}", rand::random::<u64>()));
    for name in ["a", "b"] {
        let directory = root.join("resource/illustration").join(name).join("png");
        std::fs::create_dir_all(&directory).unwrap();
        image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
            .save(directory.join("normal.png"))
            .unwrap();
    }
    let directory = root.join("resource/agents");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("characters.json"),
        r#"{"characters":{"b":"missing.md"}}"#,
    )
    .unwrap();
    let ctx = egui::Context::default();
    let mut app = Wife::from_root(&ctx, true, root.clone());
    app.codex = Some(crate::backend::codex::Client::mock("normal"));
    app.codex_character = Some("a".into());
    app.busy = true;
    app.codex_restart_pending = true;
    app.external_turn = true;
    app.input = "保留草稿".into();
    app.text = "旧角色回答".into();
    app.codex_messages.push(("old".into(), app.text.clone()));
    // Changing illustrations of the same character must keep the ongoing turn.
    app.load_picture(&ctx);
    assert!(app.busy);
    assert!(app.codex.is_some());
    app.library.next_character(&root);
    app.load_picture(&ctx);
    assert_eq!(app.codex_character.as_deref(), Some("b"));
    assert!(app.codex.is_none());
    assert!(!app.busy && !app.external_turn && !app.codex_restart_pending);
    assert!(app.text.is_empty() && app.codex_messages.is_empty());
    assert_eq!(app.input, "保留草稿");
    assert!(app.error.contains("Markdown"));
    // Even a prior startup failure cannot pin the selected character to its old session.
    app.library.next_character(&root);
    app.config.codex.executable = "lexington-fixture-unavailable.exe".into();
    app.load_picture(&ctx);
    assert_eq!(app.codex_character.as_deref(), Some("a"));
    assert_eq!(app.codex_status, "启动失败");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn window_position_persists_without_size_or_dev_mode_leaking_and_recovers_offscreen() {
    use eframe::App;
    #[derive(Default)]
    struct Storage(std::collections::HashMap<String, String>);
    impl eframe::Storage for Storage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn flush(&mut self) {}
    }
    let root = std::env::temp_dir().join(format!("lexington-position-{}", rand::random::<u64>()));
    crate::backend::config::Config::save_no_approval(&root, false).unwrap();
    let ctx = egui::Context::default();
    let run = |app: &mut Wife, position: egui::Pos2| {
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(500., 500.))),
            ..Default::default()
        };
        let viewport = raw.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.monitor_size = Some(vec2(1080., 720.));
        viewport.outer_rect = Some(Rect::from_min_size(position, vec2(500., 500.)));
        ctx.run(raw, |ctx| app.ui(ctx))
    };
    let mut storage = Storage::default();
    let mut app = Wife::from_root(&ctx, false, root.clone());
    run(&mut app, egui::Pos2::ZERO);
    let edge = pos2(-260., 480.);
    run(&mut app, edge);
    app.save(&mut storage);
    assert_eq!(
        serde_json::from_str::<[f32; 2]>(&storage.0["windowPosition"]).unwrap(),
        [edge.x, edge.y]
    );
    assert!(storage.0["window"].is_empty());

    let mut reopened = Wife::from_root(&ctx, true, root.clone());
    reopened.restore_state(&storage);
    let output = run(&mut reopened, egui::Pos2::ZERO);
    assert_eq!(
        reopened.window_position,
        Some(edge),
        "partly offscreen position must survive"
    );
    assert!(!reopened.pinned);
    assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|cmd| matches!(cmd, egui::ViewportCommand::InnerSize(size) if *size == vec2(500.,500.))));

    let mut dev = Wife::from_root(&ctx, true, root.clone());
    dev.dev = true;
    dev.restore_state(&storage);
    assert!(dev.window_position.is_none());
    let output = run(&mut dev, egui::Pos2::ZERO);
    assert_eq!(dev.window_position, Some(pos2(100., 100.)));
    assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|cmd| matches!(cmd, egui::ViewportCommand::InnerSize(size) if *size == vec2(800.,600.))));
    run(&mut dev, pos2(80., 70.));
    dev.save(&mut storage);
    assert_eq!(storage.0["devWindowPosition"], "[80.0,70.0]");
    assert_eq!(storage.0["windowPosition"], "[-260.0,480.0]");

    for value in [
        "[50000,50000]",
        "[-50000,-50000]",
        "[1e30,1e30]",
        "[null,0]",
        "invalid",
    ] {
        storage.0.insert("windowPosition".into(), value.into());
        let mut app = Wife::from_root(&ctx, true, root.clone());
        app.restore_state(&storage);
        run(&mut app, egui::Pos2::ZERO);
        assert_eq!(
            app.window_position,
            Some(pos2(560., 50.)),
            "invalid or unavailable screen: {value}"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn input_regions_exclude_blank_background_and_clear_closed_windows() {
    let root =
        std::env::temp_dir().join(format!("lexington-input-regions-{}", rand::random::<u64>()));
    crate::backend::config::Config::save_no_approval(&root, false).unwrap();
    let ctx = egui::Context::default();
    let mut app = Wife::from_root(&ctx, false, root.clone());
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    assert!(!app.input_regions.iter().any(|r| r.contains(pos2(5., 5.))));
    assert!(
        !app.input_regions
            .iter()
            .any(|r| r.contains(pos2(250., 150.)))
    );
    assert!(
        app.input_regions
            .iter()
            .any(|r| r.contains(pos2(400., 300.)))
    );
    assert!(
        app.input_regions
            .iter()
            .any(|r| r.contains(pos2(110., 175.)))
    );
    assert!(
        !app.input_regions
            .iter()
            .any(|r| r.contains(app.input_bounds.unwrap().center()))
    );
    app.settings = true;
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    assert!(
        app.input_regions
            .iter()
            .any(|r| r.contains(pos2(250., 150.)))
    );
    app.settings = false;
    frame(&mut app, &ctx, vec![]);
    assert!(
        !app.input_regions
            .iter()
            .any(|r| r.contains(pos2(250., 150.)))
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn settings_approval_checkbox_persists_without_restarting_and_controls_next_turn() {
    let root =
        std::env::temp_dir().join(format!("lexington-approval-ui-{}", rand::random::<u64>()));
    let ctx = egui::Context::default();
    crate::backend::config::Config::save_no_approval(&root, false).unwrap();
    let mut app = Wife::from_root(&ctx, false, root.clone());
    app.codex = Some(crate::backend::codex::Client::mock("normal"));
    app.settings = true;
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(800., 600.))),
            ..Default::default()
        },
        |ctx| app.ui(ctx),
    );
    let mut shapes: Vec<_> = output.shapes.iter().map(|s| &s.shape).collect();
    let mut target = None;
    while let Some(shape) = shapes.pop() {
        match shape {
            egui::Shape::Vec(children) => shapes.extend(children),
            egui::Shape::Text(text) if text.galley.job.text == "无需审批" => {
                target = Some(text.pos + vec2(8., text.galley.size().y / 2.));
                break;
            }
            _ => {}
        }
    }
    let target = target.expect("settings approval checkbox");
    for enabled in [true, false] {
        click(&mut app, &ctx, target);
        assert_eq!(app.config.codex.no_approval, enabled);
        assert_eq!(
            crate::backend::config::Config::load(&root)
                .codex
                .no_approval,
            enabled
        );
        app.input = if enabled { "no-approval" } else { "restore" }.into();
        app.submit(&ctx);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.busy && Instant::now() < deadline {
            frame(&mut app, &ctx, vec![]);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.busy);
        assert!(app.error.is_empty(), "{}", app.error);
    }
    assert!(
        app.text.starts_with("第2轮："),
        "must keep the same conversation"
    );
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn config_hot_reload_keeps_valid_settings_and_defers_connection_changes_until_idle() {
    let root = std::env::temp_dir().join(format!("lexington-hot-config-{}", rand::random::<u64>()));
    crate::backend::config::Config::save_no_approval(&root, false).unwrap();
    let path = root.join("config/config.json");
    let ctx = egui::Context::default();
    let mut app = Wife::from_root(&ctx, false, root.clone());
    app.codex = Some(crate::backend::codex::Client::mock("normal"));
    app.input = "interrupt".into();
    app.submit(&ctx);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.text.is_empty() && Instant::now() < deadline {
        frame(&mut app, &ctx, vec![]);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.busy && !app.text.is_empty());
    let mut document = serde_json::json!({"dialog":{"width":314},"clockIn":{"workReminderHours":2},"codex":{"noApproval":true}});
    std::fs::write(&path, document.to_string()).unwrap();
    app.last_config_check = Instant::now() - Duration::from_secs(1);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.config.dialog.width, 314.);
    assert_eq!(app.config.clock_in.work_reminder_hours, 2.);
    assert!(app.config.codex.no_approval && app.busy);
    assert!(
        !app.codex_restart_pending,
        "ordinary settings must preserve the conversation"
    );
    std::fs::write(&path, "{").unwrap();
    app.last_config_check = Instant::now() - Duration::from_secs(1);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.config.dialog.width, 314.);
    assert!(!app.config_error.is_empty());
    // Legacy provider fields no longer override the inherited Codex environment.
    document["codex"]["model"] = "ignored-model".into();
    document["codex"]["apiKey"] = "ignored-secret".into();
    std::fs::write(&path, document.to_string()).unwrap();
    app.reload_config();
    assert!(!app.codex_restart_pending);
    document["codex"]["developmentMode"] = true.into();
    std::fs::write(&path, document.to_string()).unwrap();
    app.reload_config();
    assert!(app.codex_restart_pending && app.busy);
    assert!(app.config.codex.development_mode);
    assert!(app.development.reset_sessions_pending);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn sidebar_switches_damage_and_format_and_refresh_preserves_selection() {
    let ctx = egui::Context::default();
    let mut app = Wife::from_context(&ctx, true);
    assert_eq!(app.library.character, "lexington");
    assert_eq!(app.library.format, "gif");
    assert!(app.picture.is_some());
    assert!(!app.library.texts.is_empty());
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(
        app.library.selected_path().unwrap().file_name().unwrap(),
        "BiKiNi_Normal.gif"
    );
    for (point, filename, damaged) in [
        (pos2(110., 225.), "BiKiNi_Damage.gif", true),
        (pos2(110., 275.), "Flower_Damage.gif", true),
        (pos2(110., 225.), "Flower_Normal.gif", false),
        (pos2(110., 275.), "Moon_Normal.gif", false),
        (pos2(110., 325.), "Moon_Normal.png", false),
        (pos2(110., 225.), "Moon_Damage.png", true),
        (pos2(110., 275.), "Normal_Damage.png", true),
    ] {
        click(&mut app, &ctx, point);
        assert_eq!(
            app.library.selected_path().unwrap().file_name().unwrap(),
            filename
        );
        assert_eq!(app.library.damaged, damaged);
        assert!(app.picture.is_some());
        let selected = app.library.selected_path().unwrap().to_path_buf();
        app.refresh(&ctx);
        assert_eq!(app.library.selected_path(), Some(selected.as_path()));
        assert_eq!(app.library.damaged, damaged);
        assert!(
            app.picture.is_some(),
            "refresh must reload the selected picture"
        );
    }
}
#[test]
fn native_pointer_events_drive_sidebar_and_pet_click_cycle() {
    let ctx = egui::Context::default();
    let mut app = Wife::from_context(&ctx, true);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, pos2(110., 175.));
    assert!(!app.pinned);
    click(&mut app, &ctx, pos2(110., 325.));
    assert_eq!(app.library.format, "png");
    click(&mut app, &ctx, pos2(110., 475.));
    assert!(app.clock_state.running());
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
    let mut app = Wife::from_context(&ctx, true);
    app.clock();
    app.clock();
    assert!(!app.clock_state.running());
    assert_eq!(app.text, "打卡已终止");
    app.config.clock_in.work_reminder_hours = 0.001;
    app.clock_state
        .toggle(&app.config.clock_in, crate::backend::clock::now() - 4)
        .unwrap();
    app.last_clock_poll = Instant::now() - Duration::from_secs(2);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.clock_state.running());
    assert_eq!(app.text, "下班时间到了！");
    app.last_activity = Instant::now() - Duration::from_secs(11);
    frame(&mut app, &ctx, vec![]);
    assert!(app.text.is_empty());
}

#[test]
fn pending_reply_stays_visible_and_pet_clicks_hide_and_restore_the_live_turn() {
    for dismiss in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        app.codex = Some(crate::backend::codex::Client::mock("normal"));
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        app.input = format!("visibility:{}", root.path().display());
        app.submit(&ctx);
        app.last_activity = Instant::now() - Duration::from_secs(60);
        frame(&mut app, &ctx, vec![egui::Event::PointerGone]);
        assert!(app.busy && app.dialog_bounds.is_some());
        assert!(app.input_bounds.is_some());
        // Neither inactivity nor a click outside may dismiss an active reply.
        click(&mut app, &ctx, pos2(750., 30.));
        assert!(app.dialog_bounds.is_some() && !app.turn_ui_hidden);
        std::fs::write(root.path().join("first"), "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.text != "首段" && Instant::now() < deadline {
            frame(&mut app, &ctx, vec![]);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.text, "首段");
        click(&mut app, &ctx, pos2(400., 300.));
        assert_eq!(app.dialog_clicks, 1);
        std::fs::write(root.path().join("next"), "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.text != "首段后续" && Instant::now() < deadline {
            frame(&mut app, &ctx, vec![]);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.text, "首段后续");
        assert_eq!(
            app.dialog_clicks, 1,
            "streaming must preserve the first click"
        );
        if dismiss {
            click(&mut app, &ctx, pos2(400., 300.));
            assert!(app.busy && app.turn_ui_hidden);
            assert!(app.dialog_bounds.is_none() && app.input_bounds.is_none());
            // One click restores the current turn while it is still streaming.
            click(&mut app, &ctx, pos2(400., 300.));
            assert!(app.busy && !app.turn_ui_hidden);
            assert_eq!(app.text, "首段后续");
            assert!(app.dialog_bounds.is_some() && app.input_bounds.is_some());
            click(&mut app, &ctx, pos2(400., 300.));
            click(&mut app, &ctx, pos2(400., 300.));
            assert!(app.busy && app.turn_ui_hidden);
        }
        std::fs::write(root.path().join("finish"), "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.busy && Instant::now() < deadline {
            frame(&mut app, &ctx, vec![]);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.busy && app.error.is_empty());
        assert!(
            app.codex_messages
                .iter()
                .any(|(_, text)| text == "首段后续最终")
        );
        if dismiss {
            assert!(app.text.is_empty());
            assert!(app.dialog_bounds.is_none() && app.input_bounds.is_none());
            // Completion updates the saved reply without reopening the panels.
            click(&mut app, &ctx, pos2(400., 300.));
            assert!(!app.turn_ui_hidden);
            assert!(app.dialog_bounds.is_some() && app.input_bounds.is_some());
        }
        assert_eq!(app.text, "首段后续最终");
        app.last_activity = Instant::now() - Duration::from_secs(60);
        frame(&mut app, &ctx, vec![]);
        assert!(
            app.text.is_empty(),
            "inactivity hiding resumes after completion"
        );
    }
}
#[test]
fn context_menu_close_receives_input_outside_artwork_and_exits() {
    let root = std::env::temp_dir().join(format!("lexington-menu-{}", rand::random::<u64>()));
    crate::backend::config::Config::save_no_approval(&root, false).unwrap();
    let ctx = egui::Context::default();
    let mut app = Wife::from_root(&ctx, false, root.clone());
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    let point = pos2(445., 325.);
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
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(800., 600.))),
            ..Default::default()
        },
        |ctx| app.ui(ctx),
    );
    let mut shapes: Vec<_> = output.shapes.iter().map(|s| &s.shape).collect();
    let mut target = None;
    while let Some(shape) = shapes.pop() {
        match shape {
            egui::Shape::Vec(children) => shapes.extend(children),
            egui::Shape::Text(text) if text.galley.job.text == "关闭" => {
                target = Some(Rect::from_min_size(text.pos, text.galley.size()));
                break;
            }
            _ => {}
        }
    }
    let close = target.expect("context menu close button");
    assert!(close.top() > 370., "close must be outside fallback artwork");
    for _ in 0..3 {
        assert!(
            app.input_regions.iter().any(|r| r.contains_rect(close)),
            "the visible close button must receive native input on every frame"
        );
        frame(&mut app, &ctx, vec![]);
    }
    let point = close.center();
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(point)]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(800., 600.))),
            events: vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        },
        |ctx| app.ui(ctx),
    );
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Close)
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn right_click_in_drag_circle_opens_context_menu() {
    for label in ["设置", "人物记忆"] {
        let ctx = egui::Context::default();
        let mut app = Wife::from_context(&ctx, true);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let point = pos2(340., 280.);
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
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(800., 600.))),
                ..Default::default()
            },
            |ctx| app.ui(ctx),
        );
        let target = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == label
                {
                    Some(text.pos + text.galley.size() / 2.)
                } else {
                    None
                }
            })
            .expect("top-level menu entry");
        click(&mut app, &ctx, target);
        assert_eq!(app.settings, label == "设置");
        assert_eq!(app.character_memory_ui.open, label == "人物记忆");
        if app.character_memory_ui.open {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                app.settings_ui(ctx);
                app.character_memories_window(ctx);
            });
            assert!(app.character_memory_ui.open);
            assert!(!app.settings && app.codex.is_none());
        }
    }
}
#[test]
fn input_hides_after_blur_and_restores_draft_and_error_on_hover_in_both_window_sizes() {
    for size in [vec2(500., 500.), vec2(800., 600.)] {
        let ctx = egui::Context::default();
        let mut app = Wife::from_context(&ctx, true);
        let mut run = |events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events,
                    ..Default::default()
                },
                |ctx| app.ui(ctx),
            );
        };
        for _ in 0..3 {
            run(vec![]);
        }
        run(vec![egui::Event::PointerMoved((size / 2.).to_pos2())]);
        for _ in 0..3 {
            run(vec![]);
        }
        let bounds = app.input_bounds.expect("input bounds");
        assert!(
            Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(bounds),
            "input must fit in viewport: {bounds:?}"
        );
        assert!(
            app.input_regions.iter().any(|r| r.contains_rect(bounds)),
            "hovering the artwork must reveal a clickable input"
        );
        let point = bounds.center();
        for pressed in [true, false] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events: vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ctx| app.ui(ctx),
            );
        }
        assert!(ctx.memory(|m| m.has_focus("message-input".into())));
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                events: vec![egui::Event::Text("草稿".into())],
                ..Default::default()
            },
            |ctx| app.ui(ctx),
        );
        assert_eq!(app.input, "草稿");
        // A native focus loss must clear the retained egui TextEdit focus.
        for _ in 0..30 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                    focused: false,
                    events: vec![egui::Event::PointerGone, egui::Event::WindowFocused(false)],
                    ..Default::default()
                },
                |ctx| app.ui(ctx),
            );
        }
        assert!(!ctx.memory(|m| m.has_focus("message-input".into())));
        let bounds = app.input_bounds.unwrap();
        assert!(
            !app.input_regions.iter().any(|r| r.contains_rect(bounds)),
            "unfocused draft must hide and stop intercepting clicks"
        );
        assert_eq!(app.input, "草稿");
        app.error = "模拟错误".into();
        for _ in 0..30 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events: vec![egui::Event::PointerGone],
                    ..Default::default()
                },
                |ctx| app.ui(ctx),
            );
        }
        assert!(
            !app.input_regions
                .iter()
                .any(|r| r.contains_rect(app.input_bounds.unwrap()))
        );
        for _ in 0..30 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events: vec![egui::Event::PointerMoved((size / 2.).to_pos2())],
                    ..Default::default()
                },
                |ctx| app.ui(ctx),
            );
        }
        assert!(
            app.input_regions
                .iter()
                .any(|r| r.contains_rect(app.input_bounds.unwrap()))
        );
        assert_eq!(app.input, "草稿");
        assert_eq!(app.error, "模拟错误");
        app.busy = true;
        for _ in 0..30 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                    focused: false,
                    events: vec![egui::Event::PointerGone],
                    ..Default::default()
                },
                |ctx| app.ui(ctx),
            );
        }
        assert!(
            app.input_regions
                .iter()
                .any(|r| r.contains_rect(app.input_bounds.unwrap())),
            "thinking indicator remains visible"
        );
    }
}
#[test]
fn input_focus_enter_error_and_escape_use_real_ui_events() {
    let ctx = egui::Context::default();
    let mut app = Wife::from_context(&ctx, true);
    app.codex = Some(crate::backend::codex::Client::mock("error"));
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
    assert!(app.busy);
    for _ in 0..100 {
        frame(&mut app, &ctx, vec![]);
        if !app.busy {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.error.contains("模拟请求失败"));
    frame(&mut app, &ctx, vec![key(egui::Key::Escape)]);
    assert!(app.input.is_empty());
    assert!(app.error.is_empty());
}
#[test]
fn sidebar_cancel_keeps_new_message_when_dialog_already_open() {
    let ctx = egui::Context::default();
    let mut app = Wife::from_context(&ctx, true);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, pos2(110., 475.));
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.text, "打卡成功！");
    click(&mut app, &ctx, pos2(110., 475.));
    assert_eq!(app.text, "打卡已终止");
    assert!(!app.clock_state.running());
    click(&mut app, &ctx, pos2(750., 30.));
    assert!(app.text.is_empty());
}
#[test]
fn enter_delivers_streamed_codex_reply_through_the_native_ui() {
    let ctx = egui::Context::default();
    let mut app = Wife::from_context(&ctx, true);
    app.codex = Some(crate::backend::codex::Client::mock("normal"));
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, pos2(400., 560.));
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::Text("你好，Codex！".into()),
            egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            },
        ],
    );
    assert!(app.busy);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.busy && Instant::now() < deadline {
        frame(&mut app, &ctx, vec![]);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!app.busy);
    assert!(app.error.is_empty(), "{}", app.error);
    assert!(app.input.is_empty());
    assert_eq!(app.text, "第1轮：你好，Codex！");
    assert!(
        app.full_text,
        "do not delay already streamed text with the typewriter"
    );
}

#[test]
#[cfg(any(target_os = "linux", windows))]
fn context_requests_wait_for_current_turn_and_send_without_replacing_drafts() {
    use crate::backend::handoff::{Endpoint, Request};
    #[cfg(target_os = "linux")]
    use std::io::{Read, Write};
    #[cfg(target_os = "linux")]
    use std::os::unix::net::UnixStream;

    let path = std::env::temp_dir().join(format!("lexington-ui-inbox-{}", rand::random::<u64>()));
    let ctx = egui::Context::default();
    let mut app = Wife::from_context(&ctx, true);
    app.inbox = Some(Endpoint::bind_at(&path).unwrap().unwrap().start(|| {}));
    app.codex = Some(crate::backend::codex::Client::mock("normal"));
    app.input = "interrupt".into();
    app.submit(&ctx);
    app.input = "未发送的草稿".into();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.text.contains("第1轮：interrupt") && Instant::now() < deadline {
        frame(&mut app, &ctx, vec![]);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.busy && app.text.contains("第1轮：interrupt"));
    let selected_path = std::env::temp_dir()
        .join("中文 空格.rs")
        .to_string_lossy()
        .into_owned();
    for text in ["第一段选区", "第二段选区"] {
        let request = Request {
            text: text.into(),
            files: vec![selected_path.clone()],
            source: "VS Code".into(),
            line: Some(17),
            workspace: None,
            diagnostics: None,
        };
        #[cfg(windows)]
        crate::backend::handoff::deliver_at(&path, &request).unwrap();
        #[cfg(target_os = "linux")]
        {
            let bytes = serde_json::to_vec(&request).unwrap();
            let mut stream = UnixStream::connect(path.join("inbox.sock")).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
                .write_all(&(bytes.len() as u32).to_le_bytes())
                .unwrap();
            stream.write_all(&bytes).unwrap();
            let mut size = [0; 4];
            stream.read_exact(&mut size).unwrap();
            let mut response = vec![0; u32::from_le_bytes(size) as usize];
            stream.read_exact(&mut response).unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&response).unwrap()["ok"],
                true
            );
        }
    }
    frame(&mut app, &ctx, vec![]);
    assert!(
        app.busy && !app.external_turn,
        "right-click must not interrupt an active turn"
    );
    app.codex.as_ref().unwrap().interrupt();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        frame(&mut app, &ctx, vec![]);
        if !app.busy && app.text.starts_with("第3轮：") {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!app.busy && app.error.is_empty(), "{}", app.error);
    assert!(app.text.starts_with("第3轮："), "{}", app.text);
    assert!(
        app.text.contains("第二段选区")
            && app
                .text
                .contains(&serde_json::to_string(&selected_path).unwrap())
    );
    assert!(app.text.contains("17"));
    assert_eq!(app.input, "未发送的草稿");
    drop(app);
    #[cfg(target_os = "linux")]
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn empty_library_character_format_fallback_and_resource_reload() {
    let fixture =
        std::env::temp_dir().join(format!("lexington-characters-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&fixture).unwrap();
    let ctx = egui::Context::default();
    let mut app = Wife::from_root(&ctx, true, fixture.clone());
    assert!(app.picture.is_none());
    assert!(app.library.selected.is_none());
    assert_eq!(app.library.format, "gif");
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
    app.refresh(&ctx);
    assert_eq!(app.library.character, "a");
    assert_eq!(app.library.format, "png");
    assert_eq!(app.library.texts, ["a的台词"]);
    assert!(app.picture.is_some());
    click(&mut app, &ctx, pos2(110., 425.));
    assert_eq!(app.library.character, "b");
    assert_eq!(app.library.format, "gif");
    assert_eq!(app.library.texts, ["b的台词"]);
    assert!(app.picture.is_some());
    click(&mut app, &ctx, pos2(110., 325.));
    assert_eq!(app.library.format, "gif"); // b only has GIF, so switching to PNG falls back.
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
    app.refresh(&ctx);
    assert_eq!(app.library.character, "a");
    assert_eq!(app.library.format, "png");
    assert_eq!(app.library.texts, ["更新后的台词"]);
    assert_eq!(app.config.dialog.width, 240.);
    assert_eq!(app.dialog_bg.as_ref().unwrap().size(), vec2(4., 3.));
    std::fs::remove_file(background).unwrap();
    std::fs::remove_file(fixture.join("resource/illustration/a/png/sample.png")).unwrap();
    app.refresh(&ctx);
    assert!(app.dialog_bg.is_none());
    assert!(app.picture.is_none());
    assert!(app.library.selected.is_none());
    assert!(app.library.character.is_empty());
    assert!(app.library.texts.is_empty());
    std::fs::remove_dir_all(fixture).unwrap();
}
#[test]
fn browser_sized_cjk_wraps_and_reserves_full_typewriter_height() {
    let ctx = egui::Context::default();
    let mut app = Wife::from_context(&ctx, true);
    app.say("一二三四五六七八九十一二三".into());
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let glyph_width = ctx.fonts(|fonts| {
        fonts
            .layout_no_wrap("一".into(), style::font(14.), Color32::BLACK)
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

#[test]
fn automatic_clock_notice_undo_settings_and_hot_reload_use_saved_state() {
    use crate::{
        backend::{
            clock::{self, ClockState},
            config::Config,
        },
        tools::activity::{Monitor, Sample},
    };
    let root = std::env::temp_dir().join(format!("lexington-clock-ui-{}", rand::random::<u64>()));
    Config::save_auto_clock(&root, true).unwrap();
    let path = root.join("clock.json");
    let ctx = egui::Context::default();
    let mut app = Wife::from_root(&ctx, false, root.clone());
    let now = clock::now();
    let tick = Instant::now();
    app.clock_state = ClockState::load(path.clone(), now - 180).unwrap();
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
        status: "正在检测工作活动".into(),
        sampled: tick,
    }));
    frame(&mut app, &ctx, vec![]);
    assert!(app.clock_notice);
    assert_eq!(app.clock_state.started(), Some(now - 180));
    assert_eq!(
        ClockState::load(path.clone(), now).unwrap().started(),
        Some(now - 180)
    );
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(800., 600.))),
            ..Default::default()
        },
        |ctx| app.ui(ctx),
    );
    let mut shapes: Vec<_> = output.shapes.iter().map(|s| &s.shape).collect();
    let mut undo = None;
    while let Some(shape) = shapes.pop() {
        match shape {
            egui::Shape::Vec(children) => shapes.extend(children),
            egui::Shape::Text(text) if text.galley.job.text == "撤销" => {
                undo = Some(text.pos + text.galley.size() / 2.);
                break;
            }
            _ => {}
        }
    }
    let undo = undo.expect("automatic clock undo button");
    assert!(app.input_regions.iter().any(|r| r.contains(undo)));
    click(&mut app, &ctx, undo);
    assert!(!app.clock_notice);
    assert!(app.clock_state.started().is_none());
    assert!(ClockState::load(path, now).unwrap().started().is_none());
    app.settings = true;
    app.settings_page = 2;
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(800., 600.))),
            ..Default::default()
        },
        |ctx| app.ui(ctx),
    );
    let mut shapes: Vec<_> = output.shapes.iter().map(|s| &s.shape).collect();
    let mut target = None;
    while let Some(shape) = shapes.pop() {
        match shape {
            egui::Shape::Vec(children) => shapes.extend(children),
            egui::Shape::Text(text) if text.galley.job.text == "自动开始工作计时" => {
                target = Some(text.pos + text.galley.size() / 2.);
                break;
            }
            _ => {}
        }
    }
    click(&mut app, &ctx, target.expect("automatic clock setting"));
    assert!(!app.config.clock_in.auto_start);
    assert!(!Config::load(&root).clock_in.auto_start);
    Config::save_auto_clock(&root, true).unwrap();
    app.reload_config();
    assert!(app.config.clock_in.auto_start);
    app.clock_edit = clock::local_time(now - 60).unwrap().label();
    app.clock_action(super::clock::ClockAction::Edit);
    assert!(app.clock_error.is_empty(), "{}", app.clock_error);
    assert_eq!(
        clock::local_time(app.clock_state.started().unwrap())
            .unwrap()
            .label(),
        app.clock_edit
    );
    app.clock_edit = "99:00".into();
    let original = app.clock_state.started();
    app.clock_action(super::clock::ClockAction::Edit);
    assert!(!app.clock_error.is_empty());
    assert_eq!(app.clock_state.started(), original);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn alarm_settings_save_edit_toggle_delete_and_reminder_requires_acknowledgement() {
    let root = std::env::temp_dir().join(format!("lexington-alarm-ui-{}", rand::random::<u64>()));
    crate::backend::config::Config::save_no_approval(&root, false).unwrap();
    let ctx = egui::Context::default();
    let mut app = Wife::from_root(&ctx, false, root.clone());
    let path = root.join("alarms.json");
    app.alarm.state = crate::backend::alarm::AlarmState::load(path.clone()).unwrap();
    app.settings = true;
    app.settings_page = 3;
    // Render the full production settings page and locate actual control text.
    let render = |app: &mut Wife| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(500., 500.))),
                ..Default::default()
            },
            |ctx| app.ui(ctx),
        )
    };
    let find = |output: &egui::FullOutput, label: &str| {
        let mut shapes: Vec<_> = output.shapes.iter().map(|s| &s.shape).collect();
        while let Some(shape) = shapes.pop() {
            match shape {
                egui::Shape::Vec(children) => shapes.extend(children),
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    return t.pos + t.galley.size() / 2.;
                }
                _ => {}
            }
        }
        panic!("missing control: {label}");
    };
    render(&mut app);
    let output = render(&mut app);
    let add = find(&output, "添加闹钟");
    let pointer = |app: &mut Wife, position, pressed| {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(500., 500.))),
                events: vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            },
            |ctx| app.ui(ctx),
        );
    };
    // Empty prompt is rejected without creating a record.
    pointer(&mut app, add, true);
    pointer(&mut app, add, false);
    assert!(app.alarm.state.alarms().is_empty());
    assert!(!app.alarm.error.is_empty());
    let now = crate::backend::clock::now();
    let time = crate::backend::clock::local_time(now + 120)
        .unwrap()
        .label();
    app.alarm
        .state
        .save(None, &time, "喝水提醒", false, now)
        .unwrap();
    render(&mut app);
    let output = render(&mut app);
    let edit = find(&output, "编辑");
    pointer(&mut app, edit, true);
    pointer(&mut app, edit, false);
    let output = render(&mut app);
    let save = find(&output, "保存闹钟");
    pointer(&mut app, save, true);
    pointer(&mut app, save, false);
    assert_eq!(
        crate::backend::alarm::AlarmState::load(path.clone())
            .unwrap()
            .alarms()[0]
            .message,
        "喝水提醒"
    );
    let due = app.alarm.state.alarms()[0].next_due;
    app.alarm.state.poll(due).unwrap();
    app.settings = false;
    render(&mut app);
    let output = render(&mut app);
    let ok = find(&output, "知道了");
    assert!(app.alarm.state.pending().len() == 1);
    // Outside clicks do not dismiss it, and the popup participates in hit testing.
    pointer(&mut app, pos2(1., 1.), true);
    pointer(&mut app, pos2(1., 1.), false);
    assert_eq!(app.alarm.state.pending().len(), 1);
    assert!(app.input_regions.iter().any(|r| r.contains(ok)));
    pointer(&mut app, ok, true);
    pointer(&mut app, ok, false);
    assert!(app.alarm.state.pending().is_empty());
    assert!(
        crate::backend::alarm::AlarmState::load(path.clone())
            .unwrap()
            .pending()
            .is_empty()
    );
    app.settings = true;
    render(&mut app);
    let output = render(&mut app);
    let toggle = find(&output, &time);
    pointer(&mut app, toggle, true);
    pointer(&mut app, toggle, false);
    assert!(app.alarm.state.alarms()[0].enabled);
    let output = render(&mut app);
    let delete = find(&output, "删除");
    pointer(&mut app, delete, true);
    pointer(&mut app, delete, false);
    assert!(app.alarm.state.alarms().is_empty());
    assert!(
        crate::backend::alarm::AlarmState::load(path)
            .unwrap()
            .alarms()
            .is_empty()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn input_keeps_typing_visible_across_multiple_cjk_characters() {
    for size in [vec2(500., 500.), vec2(800., 600.)] {
        let ctx = egui::Context::default();
        let root =
            std::env::temp_dir().join(format!("lexington-input-text-{}", rand::random::<u64>()));
        crate::backend::config::Config::save_no_approval(&root, false).unwrap();
        let mut app = Wife::from_root(&ctx, true, root.clone());
        app.library
            .refresh(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
        app.load_picture(&ctx);
        let run = |app: &mut Wife, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events,
                    ..Default::default()
                },
                |ctx| app.ui(ctx),
            )
        };
        for _ in 0..3 {
            run(&mut app, vec![]);
        }
        run(
            &mut app,
            vec![egui::Event::PointerMoved((size / 2.).to_pos2())],
        );
        for _ in 0..3 {
            run(&mut app, vec![]);
        }
        let point = app.input_bounds.unwrap().center();
        for pressed in [true, false] {
            run(
                &mut app,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert!(ctx.memory(|m| m.has_focus("message-input".into())));
        let mut expected = String::new();
        for ch in "一二三四五六七八".chars() {
            if expected.chars().count() == 3 {
                app.error = "模拟连接中断，请稍后重试。".into();
            }
            expected.push(ch);
            let output = run(&mut app, vec![egui::Event::Text(ch.to_string())]);
            assert_eq!(app.input, expected);
            let mut shapes: Vec<_> = output
                .shapes
                .iter()
                .map(|s| (s.clip_rect, &s.shape))
                .collect();
            let mut text = None;
            while let Some((clip, shape)) = shapes.pop() {
                match shape {
                    egui::Shape::Vec(children) => shapes.extend(children.iter().map(|s| (clip, s))),
                    egui::Shape::Text(shape) if shape.galley.job.text == expected => {
                        text = Some((clip, Rect::from_min_size(shape.pos, shape.galley.size())));
                        break;
                    }
                    _ => {}
                }
            }
            let (clip, text) = text.expect("typed text must be painted");
            assert!(
                clip.expand(1.).contains_rect(text),
                "short draft must remain visible: {expected}, clip={clip:?}, text={text:?}, size={size:?}"
            );
        }
        // Continue past the single-line width: the caret and tail must scroll into view.
        for _ in 0..8 {
            let output = run(&mut app, vec![egui::Event::Text("九十".into())]);
            let mut shapes: Vec<_> = output
                .shapes
                .iter()
                .map(|s| (s.clip_rect, &s.shape))
                .collect();
            let mut tail = None;
            while let Some((clip, shape)) = shapes.pop() {
                match shape {
                    egui::Shape::Vec(children) => shapes.extend(children.iter().map(|s| (clip, s))),
                    egui::Shape::Text(shape) if shape.galley.job.text == app.input => {
                        let right = shape.pos.x + shape.galley.size().x;
                        tail = Some((clip, right));
                        break;
                    }
                    _ => {}
                }
            }
            let (clip, right) = tail.expect("long draft must be painted");
            assert!(
                right <= clip.right() + 1. && right >= clip.left() + 10.,
                "long input tail clipped: right={right}, clip={clip:?}"
            );
        }
        let before = app.input.clone();
        run(&mut app, vec![egui::Event::Ime(egui::ImeEvent::Enabled)]);
        run(
            &mut app,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit("ceshi".into()))],
        );
        run(
            &mut app,
            vec![egui::Event::Ime(egui::ImeEvent::Commit(
                "测试输入法".into(),
            ))],
        );
        assert_eq!(app.input, format!("{before}测试输入法"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
