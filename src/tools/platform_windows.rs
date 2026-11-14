use super::DragAnchor;
use eframe::egui::{self, Rect};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT},
    Graphics::{
        Dwm::{DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND, DwmEnableBlurBehindWindow},
        Gdi::{
            ClientToScreen, CreateRectRgn, DeleteObject, EnumDisplayMonitors, GetMonitorInfoW, HDC,
            HMONITOR, MONITORINFO, ScreenToClient,
        },
    },
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON},
        WindowsAndMessaging::*,
    },
};

fn is_passthrough(pointer: egui::Pos2, regions: &[Rect], screen: Rect, dragging: bool) -> bool {
    let pointer = pointer + screen.min.to_vec2();
    !dragging
        && !regions.iter().any(|region| {
            let clipped = region.intersect(screen);
            clipped.is_positive() && clipped.contains(pointer)
        })
}

fn input_window_style(style: isize, passthrough: bool) -> isize {
    let base = (style & !(WS_EX_APPWINDOW as isize)) | WS_EX_TOOLWINDOW as isize;
    if passthrough {
        base | (WS_EX_TRANSPARENT | WS_EX_LAYERED) as isize
    } else {
        base & !((WS_EX_TRANSPARENT | WS_EX_LAYERED) as isize)
    }
}

pub fn configure(_: &eframe::CreationContext<'_>) {}

fn restore_transparency(window: HWND) {
    unsafe {
        let region = CreateRectRgn(0, 0, -1, -1);
        if region.is_null() {
            return;
        }
        let blur = DWM_BLURBEHIND {
            dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
            fEnable: 1,
            hRgnBlur: region,
            fTransitionOnMaximized: 0,
        };
        DwmEnableBlurBehindWindow(window, &blur);
        DeleteObject(region);
    }
}

pub struct DragWindow {
    window: HWND,
    drag: Option<DragAnchor>,
}
impl DragWindow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let RawWindowHandle::Win32(handle) = cc.window_handle().ok()?.as_raw() else {
            return None;
        };
        Some(Self {
            window: handle.hwnd.get() as HWND,
            drag: None,
        })
    }
    pub fn configure(&self) {
        unsafe {
            let style = GetWindowLongPtrW(self.window, GWL_EXSTYLE);
            SetWindowLongPtrW(
                self.window,
                GWL_EXSTYLE,
                (style & !(WS_EX_APPWINDOW as isize)) | WS_EX_TOOLWINDOW as isize,
            );
            SetWindowPos(
                self.window,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            restore_transparency(self.window);
        }
    }
    pub fn focus(&self) {
        unsafe {
            SetForegroundWindow(self.window);
        }
    }
    fn pointer(&self) -> Option<POINT> {
        let mut point = POINT { x: 0, y: 0 };
        (unsafe { GetCursorPos(&mut point) } != 0).then_some(point)
    }
    fn origin(&self) -> Option<POINT> {
        let mut point = POINT { x: 0, y: 0 };
        (unsafe { ClientToScreen(self.window, &mut point) } != 0).then_some(point)
    }
    pub fn pointer_position(&self, scale: f32) -> Option<egui::Pos2> {
        let mut point = self.pointer()?;
        (unsafe { ScreenToClient(self.window, &mut point) } != 0)
            .then(|| egui::pos2(point.x as f32, point.y as f32) / scale)
    }
    pub fn position(&self, scale: f32) -> Option<egui::Pos2> {
        let origin = self.origin()?;
        Some(egui::pos2(origin.x as f32, origin.y as f32) / scale)
    }
    pub fn screen_pointer(&self, scale: f32) -> Option<(egui::Pos2, bool)> {
        let pointer = self.pointer()?;
        Some((
            egui::pos2(pointer.x as f32, pointer.y as f32) / scale,
            unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } < 0,
        ))
    }
    pub fn window_at_pointer(
        &self,
        title: &str,
        scale: f32,
    ) -> Option<(super::WindowId, egui::Pos2)> {
        unsafe {
            let window = GetAncestor(WindowFromPoint(self.pointer()?), GA_ROOT);
            let mut owner = 0;
            GetWindowThreadProcessId(window, &mut owner);
            if owner != std::process::id() {
                return None;
            }
            let mut text = [0u16; 256];
            let length = GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32);
            if String::from_utf16_lossy(&text[..length as usize]) != title {
                return None;
            }
            let mut origin = POINT { x: 0, y: 0 };
            if ClientToScreen(window, &mut origin) == 0 {
                return None;
            }
            Some((
                window as usize,
                egui::pos2(origin.x as f32, origin.y as f32) / scale,
            ))
        }
    }
    pub fn move_window(&self, window: super::WindowId, position: egui::Pos2, scale: f32) {
        let position = position * scale;
        unsafe {
            SetWindowPos(
                window as HWND,
                std::ptr::null_mut(),
                position.x.round() as i32,
                position.y.round() as i32,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    pub fn monitors(&self, scale: f32) -> Vec<Rect> {
        unsafe extern "system" fn collect(
            monitor: HMONITOR,
            _: HDC,
            _: *mut RECT,
            data: LPARAM,
        ) -> i32 {
            let monitors = unsafe { &mut *(data as *mut Vec<(bool, RECT)>) };
            let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
            info.cbSize = size_of_val(&info) as u32;
            if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
                monitors.push((info.dwFlags & MONITORINFOF_PRIMARY != 0, info.rcMonitor));
            }
            1
        }
        let mut monitors: Vec<(bool, RECT)> = Vec::new();
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(collect),
                &mut monitors as *mut _ as LPARAM,
            );
        }
        monitors.sort_by_key(|(primary, _)| !primary);
        monitors
            .into_iter()
            .map(|(_, rect)| {
                Rect::from_min_max(
                    egui::pos2(rect.left as f32, rect.top as f32) / scale,
                    egui::pos2(rect.right as f32, rect.bottom as f32) / scale,
                )
            })
            .collect()
    }
    pub fn arm(&mut self, scale: f32) {
        self.drag = self.origin().zip(self.pointer()).map(|(origin, pointer)| {
            DragAnchor::new(
                (origin.x, origin.y),
                (pointer.x - origin.x, pointer.y - origin.y),
                scale,
            )
        });
    }
    pub fn update(&mut self) -> bool {
        if unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } >= 0 {
            self.drag = None;
            return false;
        }
        let pointer = self.pointer();
        let Some(drag) = &mut self.drag else {
            return false;
        };
        let Some(pointer) = pointer else {
            self.drag = None;
            return false;
        };
        if let Some((left, top)) = drag.position((pointer.x, pointer.y)) {
            unsafe {
                SetWindowPos(
                    self.window,
                    std::ptr::null_mut(),
                    left,
                    top,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
        drag.active
    }
    pub fn set_input_regions(&mut self, regions: &[Rect], screen: Rect, scale: f32) {
        let Some(pointer) = self.pointer_position(scale) else {
            return;
        };
        let passthrough = is_passthrough(pointer, regions, screen, self.drag.is_some());
        unsafe {
            let style = GetWindowLongPtrW(self.window, GWL_EXSTYLE);
            let updated = input_window_style(style, passthrough);
            if updated != style {
                SetWindowLongPtrW(self.window, GWL_EXSTYLE, updated);
                restore_transparency(self.window);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{input_window_style, is_passthrough};
    use eframe::egui::{Rect, pos2};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_EX_TRANSPARENT,
    };

    #[test]
    fn windows_hit_test_passes_through_empty_regions() {
        let screen = Rect::from_min_size(pos2(0., 0.), [500., 500.].into());
        let regions = [Rect::from_min_max(pos2(100., 100.), pos2(200., 200.))];
        assert!(is_passthrough(pos2(10., 10.), &regions, screen, false));
        assert!(!is_passthrough(pos2(150., 150.), &regions, screen, false));
        assert!(is_passthrough(pos2(150., 150.), &[], screen, false));
    }

    #[test]
    fn windows_hit_test_clips_regions_and_ignores_empty_bounds() {
        let screen = Rect::from_min_max(pos2(0., 0.), pos2(100., 100.));
        let regions = [
            Rect::from_min_max(pos2(80., 80.), pos2(120., 120.)),
            Rect::from_min_max(pos2(20., 20.), pos2(20., 40.)),
            Rect::NOTHING,
        ];
        assert!(!is_passthrough(pos2(90., 90.), &regions, screen, false));
        assert!(is_passthrough(pos2(110., 110.), &regions, screen, false));
        assert!(is_passthrough(pos2(20., 30.), &regions, screen, false));
    }

    #[test]
    fn windows_hit_test_converts_client_points_to_viewport_coordinates() {
        let screen = Rect::from_min_max(pos2(50., 100.), pos2(250., 300.));
        let regions = [Rect::from_min_max(pos2(100., 150.), pos2(150., 200.))];
        for scale in [1., 1.25, 1.5, 2.] {
            let physical_pointer = pos2(75., 75.) * scale;
            assert!(!is_passthrough(
                physical_pointer / scale,
                &regions,
                screen,
                false
            ));
            assert!(is_passthrough(pos2(10., 10.), &regions, screen, false));
        }
    }

    #[test]
    fn windows_hit_test_keeps_input_during_drag() {
        let screen = Rect::from_min_max(pos2(0., 0.), pos2(100., 100.));
        assert!(!is_passthrough(pos2(10., 10.), &[], screen, true));
        assert!(is_passthrough(pos2(10., 10.), &[], screen, false));
    }

    #[test]
    fn windows_input_style_uses_layered_transparency_and_preserves_other_flags() {
        let preserved = (WS_EX_TOPMOST | WS_EX_NOACTIVATE) as isize;
        let transparent = input_window_style(preserved | WS_EX_APPWINDOW as isize, true);
        let expected = preserved | (WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_LAYERED) as isize;
        assert_eq!(transparent, expected);
        assert_eq!(input_window_style(transparent, true), expected);
        assert_eq!(
            input_window_style(transparent, false),
            preserved | WS_EX_TOOLWINDOW as isize
        );
    }

    #[test]
    fn windows_input_style_reasserts_flags_after_external_style_changes() {
        let style = input_window_style(WS_EX_APPWINDOW as isize, true);
        for removed in [WS_EX_TRANSPARENT, WS_EX_LAYERED, WS_EX_TOOLWINDOW] {
            assert_eq!(input_window_style(style & !(removed as isize), true), style);
        }
        assert_eq!(
            input_window_style(style | WS_EX_APPWINDOW as isize, true),
            style
        );
    }
}
