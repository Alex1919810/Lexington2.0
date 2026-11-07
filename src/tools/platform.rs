// Native Linux window-manager hints absent from egui's Windows-only taskbar flag.
#[cfg(target_os = "linux")]
pub fn configure(cc: &eframe::CreationContext<'_>) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = cc.window_handle() else {
        return;
    };
    let window = match handle.as_raw() {
        RawWindowHandle::Xlib(h) => h.window as u32,
        RawWindowHandle::Xcb(h) => h.window.get(),
        _ => return,
    };
    if let Err(error) = x11_hints(window) {
        eprintln!("设置桌宠窗口属性失败: {error}");
    }
}
#[cfg(target_os = "linux")]
fn x11_hints(window: u32) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::{connection::Connection, protocol::xproto::*, wrapper::ConnectionExt as _};
    let (conn, screen) = x11rb::connect(None)?;
    let atom = |name: &[u8]| -> Result<u32, Box<dyn std::error::Error>> {
        Ok(conn.intern_atom(false, name)?.reply()?.atom)
    };
    let state = atom(b"_NET_WM_STATE")?;
    let skip_taskbar = atom(b"_NET_WM_STATE_SKIP_TASKBAR")?;
    let skip_pager = atom(b"_NET_WM_STATE_SKIP_PAGER")?;
    let above = atom(b"_NET_WM_STATE_ABOVE")?;
    let mut states: Vec<_> = conn
        .get_property(false, window, state, AtomEnum::ATOM, 0, 32)?
        .reply()?
        .value32()
        .into_iter()
        .flatten()
        .collect();
    for value in [skip_taskbar, skip_pager, above] {
        if !states.contains(&value) {
            states.push(value);
        }
    }
    conn.change_property32(PropMode::REPLACE, window, state, AtomEnum::ATOM, &states)?
        .check()?;
    let event = ClientMessageEvent::new(32, window, state, [1, skip_taskbar, skip_pager, 1, 0]);
    conn.send_event(
        false,
        conn.setup().roots[screen].root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    )?
    .check()?;
    conn.send_event(
        false,
        conn.setup().roots[screen].root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        ClientMessageEvent::new(32, window, state, [1, above, 0, 1, 0]),
    )?
    .check()?;
    conn.flush()?;

    Ok(())
}
#[cfg(not(target_os = "linux"))]
pub fn configure(_: &eframe::CreationContext<'_>) {}

#[cfg(any(target_os = "linux", test))]
/// Snapshot the press in screen pixels once. Window-relative pointer deltas create
/// feedback after each move; capturing only after the threshold also loses pixels.
#[derive(Clone, Copy)]
struct DragAnchor {
    offset: (i32, i32),
    press: (i32, i32),
    threshold: f32,
    active: bool,
}
#[cfg(any(target_os = "linux", test))]
impl DragAnchor {
    fn new(window: (i32, i32), offset: (i32, i32), pixels_per_point: f32) -> Self {
        Self {
            offset,
            press: (window.0 + offset.0, window.1 + offset.1),
            threshold: 5. * pixels_per_point,
            active: false,
        }
    }
    fn position(&mut self, pointer: (i32, i32)) -> Option<(i32, i32)> {
        let dx = (pointer.0 - self.press.0) as f32;
        let dy = (pointer.1 - self.press.1) as f32;
        self.active |= dx.hypot(dy) > self.threshold;
        self.active
            .then_some((pointer.0 - self.offset.0, pointer.1 - self.offset.1))
    }
}

#[cfg(target_os = "linux")]
pub struct DragWindow {
    connection: x11rb::rust_connection::RustConnection,
    window: u32,
    root: u32,
    move_atom: u32,
    window_manager: bool,
    drag: Option<DragAnchor>,
    input_shape: Vec<x11rb::protocol::xproto::Rectangle>,
    shape_available: bool,
}
#[cfg(target_os = "linux")]
impl DragWindow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use x11rb::{connection::Connection, protocol::xproto::ConnectionExt};
        let window = match cc.window_handle().ok()?.as_raw() {
            RawWindowHandle::Xlib(h) => h.window as u32,
            RawWindowHandle::Xcb(h) => h.window.get(),
            _ => return None,
        };
        let (connection, screen) = x11rb::connect(None).ok()?;
        let root = connection.setup().roots[screen].root;
        let move_atom = connection
            .intern_atom(false, b"_NET_MOVERESIZE_WINDOW")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let window_manager = connection
            .intern_atom(false, b"_NET_SUPPORTING_WM_CHECK")
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|atom| {
                connection
                    .get_property(
                        false,
                        root,
                        atom.atom,
                        x11rb::protocol::xproto::AtomEnum::WINDOW,
                        0,
                        1,
                    )
                    .ok()
            })
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|reply| {
                reply
                    .value32()
                    .is_some_and(|mut values| values.next().is_some())
            });
        use x11rb::protocol::shape::ConnectionExt as _;
        let shape_available = connection
            .shape_query_version()
            .ok()
            .and_then(|reply| reply.reply().ok())
            .is_some_and(|v| (v.major_version, v.minor_version) >= (1, 1));
        Some(Self {
            connection,
            window,
            root,
            move_atom,
            window_manager,
            drag: None,
            input_shape: Vec::new(),
            shape_available,
        })
    }
    pub fn pointer_position(&self, pixels_per_point: f32) -> Option<eframe::egui::Pos2> {
        use x11rb::protocol::xproto::ConnectionExt;
        let pointer = self
            .connection
            .query_pointer(self.window)
            .ok()?
            .reply()
            .ok()?;
        pointer.same_screen.then(|| {
            eframe::egui::pos2(
                f32::from(pointer.win_x) / pixels_per_point,
                f32::from(pointer.win_y) / pixels_per_point,
            )
        })
    }
    pub fn screen_pointer(&self, pixels_per_point: f32) -> Option<(eframe::egui::Pos2, bool)> {
        use x11rb::protocol::xproto::{ConnectionExt, KeyButMask};
        let pointer = self
            .connection
            .query_pointer(self.root)
            .ok()?
            .reply()
            .ok()?;
        pointer.same_screen.then(|| {
            (
                eframe::egui::pos2(
                    f32::from(pointer.root_x) / pixels_per_point,
                    f32::from(pointer.root_y) / pixels_per_point,
                ),
                pointer.mask.contains(KeyButMask::BUTTON1),
            )
        })
    }
    /// Resolve the actual native child under a new press, rather than relying
    /// on egui's local pointer/outer-rectangle cache after moving a viewport.
    pub fn window_at_pointer(
        &self,
        title: &str,
        pixels_per_point: f32,
    ) -> Option<(u32, eframe::egui::Pos2)> {
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
        let name = self
            .connection
            .intern_atom(false, b"_NET_WM_NAME")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let pid = self
            .connection
            .intern_atom(false, b"_NET_WM_PID")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let mut window = self.root;
        for _ in 0..8 {
            window = self
                .connection
                .query_pointer(window)
                .ok()?
                .reply()
                .ok()?
                .child;
            if window == x11rb::NONE {
                return None;
            }
            let owner = self
                .connection
                .get_property(false, window, pid, AtomEnum::CARDINAL, 0, 1)
                .ok()?
                .reply()
                .ok()?;
            if owner.value32().and_then(|mut values| values.next()) != Some(std::process::id()) {
                continue;
            }
            let value = self
                .connection
                .get_property(false, window, name, AtomEnum::ANY, 0, 256)
                .ok()?
                .reply()
                .ok()?;
            if value.value != title.as_bytes() {
                continue;
            }
            let position = self
                .connection
                .translate_coordinates(window, self.root, 0, 0)
                .ok()?
                .reply()
                .ok()?;
            return Some((
                window,
                eframe::egui::pos2(f32::from(position.dst_x), f32::from(position.dst_y))
                    / pixels_per_point,
            ));
        }
        None
    }
    pub fn move_window(&self, window: u32, position: eframe::egui::Pos2, pixels_per_point: f32) {
        use x11rb::protocol::xproto::{ClientMessageEvent, ConnectionExt, EventMask};
        let position = position * pixels_per_point;
        if !self.window_manager {
            // A bare X server has no WM to consume the EWMH message.
            if let Ok(cookie) = self.connection.configure_window(
                window,
                &x11rb::protocol::xproto::ConfigureWindowAux::new()
                    .x(position.x.round() as i32)
                    .y(position.y.round() as i32),
            ) {
                let _ = cookie.check();
            }
            return;
        }
        let event = ClientMessageEvent::new(
            32,
            window,
            self.move_atom,
            [
                1 | (1 << 8) | (1 << 9) | (1 << 12),
                position.x.round() as i32 as u32,
                position.y.round() as i32 as u32,
                0,
                0,
            ],
        );
        // A checked request is necessary when the connection may be dropped
        // immediately after closing this child viewport.
        if let Ok(cookie) = self.connection.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        ) {
            let _ = cookie.check();
        }
    }
    pub fn position(&self, pixels_per_point: f32) -> Option<eframe::egui::Pos2> {
        use x11rb::protocol::xproto::ConnectionExt;
        let position = self
            .connection
            .translate_coordinates(self.window, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        Some(eframe::egui::pos2(
            f32::from(position.dst_x) / pixels_per_point,
            f32::from(position.dst_y) / pixels_per_point,
        ))
    }
    pub fn monitors(&self, pixels_per_point: f32) -> Vec<eframe::egui::Rect> {
        use eframe::egui::{Rect, pos2, vec2};
        use x11rb::protocol::{randr::ConnectionExt as _, xproto::ConnectionExt as _};
        if let Some(mut monitors) = self
            .connection
            .randr_get_monitors(self.root, true)
            .ok()
            .and_then(|reply| reply.reply().ok())
            .map(|reply| reply.monitors)
        {
            monitors.sort_by_key(|monitor| !monitor.primary);
            let monitors: Vec<_> = monitors
                .into_iter()
                .filter(|m| m.width > 0 && m.height > 0)
                .map(|m| {
                    Rect::from_min_size(
                        pos2(f32::from(m.x), f32::from(m.y)) / pixels_per_point,
                        vec2(f32::from(m.width), f32::from(m.height)) / pixels_per_point,
                    )
                })
                .collect();
            if !monitors.is_empty() {
                return monitors;
            }
        }
        self.connection
            .get_geometry(self.root)
            .ok()
            .and_then(|reply| reply.reply().ok())
            .map(|geometry| {
                vec![Rect::from_min_size(
                    eframe::egui::Pos2::ZERO,
                    vec2(f32::from(geometry.width), f32::from(geometry.height)) / pixels_per_point,
                )]
            })
            .unwrap_or_default()
    }
    pub fn set_input_regions(
        &mut self,
        regions: &[eframe::egui::Rect],
        screen: eframe::egui::Rect,
        pixels_per_point: f32,
    ) {
        use x11rb::protocol::{
            shape::{ConnectionExt, SK, SO},
            xproto::{ClipOrdering, Rectangle},
        };
        if !self.shape_available {
            return;
        }
        let rectangles: Vec<_> = regions
            .iter()
            .filter_map(|rect| {
                let rect = rect.intersect(screen);
                if !rect.is_positive() {
                    return None;
                }
                let min = ((rect.min - screen.min) * pixels_per_point).floor();
                let max = ((rect.max - screen.min) * pixels_per_point).ceil();
                Some(Rectangle {
                    x: min.x as i16,
                    y: min.y as i16,
                    width: (max.x - min.x) as u16,
                    height: (max.y - min.y) as u16,
                })
            })
            .collect();
        if self.input_shape.len() == rectangles.len()
            && self
                .input_shape
                .iter()
                .zip(&rectangles)
                .all(|(a, b)| (a.x, a.y, a.width, a.height) == (b.x, b.y, b.width, b.height))
        {
            return;
        }
        match self
            .connection
            .shape_rectangles(
                SO::SET,
                SK::INPUT,
                ClipOrdering::UNSORTED,
                self.window,
                0,
                0,
                &rectangles,
            )
            .map_err(|error| error.to_string())
            .and_then(|cookie| cookie.check().map_err(|error| error.to_string()))
        {
            Ok(()) => self.input_shape = rectangles,
            Err(error) => eprintln!("更新鼠标输入区域失败: {error}"),
        }
    }
    pub fn configure(&self) {
        if let Err(error) = x11_hints(self.window) {
            eprintln!("设置桌宠窗口属性失败: {error}");
        }
    }
    pub fn focus(&self) {
        use x11rb::protocol::xproto::{ConnectionExt, InputFocus};
        // Desktop overlays are not automatically focused by every window manager.
        if let Ok(cookie) =
            self.connection
                .set_input_focus(InputFocus::PARENT, self.window, x11rb::CURRENT_TIME)
        {
            let _ = cookie.check();
        }
    }
    pub fn arm(&mut self, pixels_per_point: f32) {
        use x11rb::protocol::xproto::ConnectionExt;
        self.drag = (|| {
            let window = self
                .connection
                .translate_coordinates(self.window, self.root, 0, 0)
                .ok()?
                .reply()
                .ok()?;
            let pointer = self
                .connection
                .query_pointer(self.root)
                .ok()?
                .reply()
                .ok()?;
            // After a move under a stationary pointer, egui's cached local cursor may
            // predate ConfigureNotify. Capture both coordinates from X11 when pressed.
            Some(DragAnchor::new(
                (window.dst_x.into(), window.dst_y.into()),
                (
                    i32::from(pointer.root_x) - i32::from(window.dst_x),
                    i32::from(pointer.root_y) - i32::from(window.dst_y),
                ),
                pixels_per_point,
            ))
        })();
    }
    pub fn update(&mut self) -> bool {
        use x11rb::{
            connection::Connection,
            protocol::xproto::{ClientMessageEvent, ConnectionExt, EventMask, KeyButMask},
        };
        let Some(drag) = &mut self.drag else {
            return false;
        };
        let Some(pointer) = self
            .connection
            .query_pointer(self.root)
            .ok()
            .and_then(|reply| reply.reply().ok())
        else {
            self.drag = None;
            return false;
        };
        if !pointer.mask.contains(KeyButMask::BUTTON1) {
            self.drag = None;
            return false;
        }
        if let Some((x, y)) = drag.position((pointer.root_x.into(), pointer.root_y.into())) {
            let event = ClientMessageEvent::new(
                32,
                self.window,
                self.move_atom,
                [
                    1 | (1 << 8) | (1 << 9) | (1 << 12),
                    x as u32,
                    y as u32,
                    0,
                    0,
                ],
            );
            let _ = self.connection.send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            );
            let _ = self.connection.flush();
        }
        drag.active
    }
}
#[cfg(test)]
mod tests {
    use super::DragAnchor;
    #[test]
    fn drag_preserves_press_offset_threshold_and_dpi_without_accumulation() {
        for scale in [1., 1.5, 2.] {
            let mut drag = DragAnchor::new(
                (100, 100),
                ((340. * scale) as i32, (280. * scale) as i32),
                scale,
            );
            let press = drag.press;
            assert_eq!(drag.position((press.0 + 3, press.1 + 2)), None);
            assert_eq!(
                drag.position((press.0 + 80, press.1 + 40)),
                Some((180, 140))
            );
            assert_eq!(
                drag.position((press.0 + 100, press.1 + 50)),
                Some((200, 150))
            );
            // Crossing the origin and leaving the original window must keep the same anchor.
            assert_eq!(
                drag.position((press.0 - 400, press.1 - 250)),
                Some((-300, -150))
            );
            assert_eq!(drag.position(press), Some((100, 100)));
        }
    }
}
