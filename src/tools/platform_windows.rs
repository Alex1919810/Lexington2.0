use super::DragAnchor;
use eframe::egui::{self, Rect};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT},
    Graphics::Gdi::{
        ClientToScreen, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
        ScreenToClient,
    },
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON},
        WindowsAndMessaging::*,
    },
};

pub fn configure(_: &eframe::CreationContext<'_>) {}

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
        let passthrough = self.drag.is_none()
            && !regions
                .iter()
                .any(|region| region.intersect(screen).contains(pointer));
        unsafe {
            let style = GetWindowLongPtrW(self.window, GWL_EXSTYLE);
            let base = (style & !(WS_EX_APPWINDOW as isize)) | WS_EX_TOOLWINDOW as isize;
            let updated = if passthrough {
                base | (WS_EX_TRANSPARENT | WS_EX_LAYERED) as isize
            } else {
                base & !((WS_EX_TRANSPARENT | WS_EX_LAYERED) as isize)
            };
            if updated != style {
                SetWindowLongPtrW(self.window, GWL_EXSTYLE, updated);
            }
        }
    }
}
