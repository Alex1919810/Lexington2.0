#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod backend;
mod tools;
mod ui;

use eframe::egui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use backend::handoff::{self, Request};
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "--native-messaging") {
        handoff::native_host()?;
        return Ok(());
    }
    if args.get(1).is_some_and(|arg| arg == "--ask-json") {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::io::stdin()
            .take((handoff::MAX_MESSAGE + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > handoff::MAX_MESSAGE {
            return Err("选中内容超过 1 MiB，请缩小选区".into());
        }
        handoff::deliver(serde_json::from_slice(&bytes)?)?;
        return Ok(());
    }
    if args.get(1).is_some_and(|arg| arg == "--ask-files") {
        let files = args
            .iter()
            .skip(2)
            .map(|arg| {
                std::fs::canonicalize(arg)?
                    .into_os_string()
                    .into_string()
                    .map_err(|_| std::io::Error::other("文件路径不是有效 UTF-8"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        handoff::deliver(Request {
            files,
            source: "文件管理器".into(),
            ..Default::default()
        })?;
        return Ok(());
    }
    let dev = std::env::args().any(|arg| arg == "--dev");
    #[cfg(target_os = "linux")]
    let Some(endpoint) = handoff::Endpoint::bind(dev)? else {
        // Context-menu requests are forwarded to this existing instance.
        return Ok(());
    };
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
    if let Ok(icon) = image::open(tools::paths::resource_root().join("resource/icon/Normal.ico")) {
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
            // Mutter constrains Utility windows by their full transparent rectangle.
            // A desktop overlay can cross monitor edges; omit struts so it reserves no space.
            attributes.with_window_type(egui::X11WindowType::Dock)
        }));
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_x11();
        }));
    }
    eframe::run_native(
        "Lexington",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::Pet::new(
                cc,
                #[cfg(target_os = "linux")]
                endpoint,
            )))
        }),
    )?;
    Ok(())
}
