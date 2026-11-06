use super::{app::Pet, style};
use eframe::egui::{self, Color32, Rect, pos2, vec2};
use std::time::{Duration, Instant};
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
fn settings_approval_checkbox_persists_without_restarting_and_controls_next_turn() {
    let root =
        std::env::temp_dir().join(format!("lexington-approval-ui-{}", rand::random::<u64>()));
    let ctx = egui::Context::default();
    crate::backend::config::Config::save_no_approval(&root, false).unwrap();
    let mut app = Pet::from_root(&ctx, false, root.clone());
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
    let mut app = Pet::from_root(&ctx, false, root.clone());
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
    document["codex"]["executable"] = root.join("missing-codex").to_str().unwrap().into();
    std::fs::write(&path, document.to_string()).unwrap();
    app.last_config_check = Instant::now() - Duration::from_secs(1);
    frame(&mut app, &ctx, vec![]);
    assert!(app.codex_restart_pending && app.busy && app.codex.is_some());
    assert!(app.config_error.is_empty());
    app.codex.as_ref().unwrap().interrupt();
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.codex_restart_pending && Instant::now() < deadline {
        frame(&mut app, &ctx, vec![]);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!app.busy && !app.codex_restart_pending);
    assert!(
        app.codex.is_none(),
        "the updated executable must be used after the old turn ends"
    );
    assert_eq!(app.codex_status, "启动失败");
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn real_resources_select_cycle_refresh_and_change_format() {
    let ctx = egui::Context::default();
    let mut app = Pet::from_context(&ctx, true);
    assert_eq!(app.library.illustrations.len(), 30);
    assert_eq!(app.library.character, "lexington");
    assert_eq!(app.library.format, "gif");
    assert!(app.picture.is_some());
    assert!(!app.library.texts.is_empty());
    let old = app.library.illustrations[app.library.selected.unwrap()]
        .path
        .clone();
    app.refresh(&ctx);
    assert_eq!(
        app.library.illustrations[app.library.selected.unwrap()].path,
        old
    );
    app.library.format = "png".into();
    app.library.select_first();
    app.load_picture(&ctx);
    assert_eq!(
        app.library.illustrations[app.library.selected.unwrap()].format,
        "png"
    );
    assert!(app.picture.is_some());
}
#[test]
fn sidebar_switches_matching_damage_versions_and_separate_next_queues() {
    let ctx = egui::Context::default();
    let mut app = Pet::from_context(&ctx, true);
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
    }
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
    assert_eq!(app.library.format, "png");
    click(&mut app, &ctx, pos2(110., 425.));
    assert!(app.clock_state.started.is_some());
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
    assert!(app.clock_state.started.is_none());
    assert_eq!(app.text, "打卡已终止");
    app.config.clock_in.work_reminder_hours = 0.001;
    app.clock_state.started = Some(Instant::now() - Duration::from_secs(4));
    frame(&mut app, &ctx, vec![]);
    assert!(app.clock_state.started.is_none());
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
    let mut app = Pet::from_context(&ctx, true);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, pos2(110., 425.));
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.text, "打卡成功！");
    click(&mut app, &ctx, pos2(110., 425.));
    assert_eq!(app.text, "打卡已终止");
    assert!(app.clock_state.started.is_none());
    click(&mut app, &ctx, pos2(750., 30.));
    assert!(app.text.is_empty());
}
#[test]
fn enter_delivers_streamed_codex_reply_through_the_native_ui() {
    let ctx = egui::Context::default();
    let mut app = Pet::from_context(&ctx, true);
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
fn empty_library_character_format_fallback_and_resource_reload() {
    let fixture =
        std::env::temp_dir().join(format!("lexington-characters-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&fixture).unwrap();
    let ctx = egui::Context::default();
    let mut app = Pet::from_root(&ctx, true, fixture.clone());
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
    click(&mut app, &ctx, pos2(110., 375.));
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
    let mut app = Pet::from_context(&ctx, true);
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
