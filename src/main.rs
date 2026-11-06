#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod backend;
mod tools;
mod ui;

use eframe::egui;

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
        Box::new(|cc| Ok(Box::new(ui::Pet::new(cc)))),
    )
}
