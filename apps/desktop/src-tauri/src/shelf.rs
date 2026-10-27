//! What is on the screen besides the pet: other apps' windows, which
//! the pet can land on, and whether the app in front is full screen,
//! in which case the pet gets out of the way. Window frames need no
//! permission on either platform; titles would, so they are not read.

use serde::Serialize;

/// One window of another app, in the pet window's physical pixel
/// space (the one `outer_position` uses), origin top left.
// Never built where there is no window list (Linux), which `-D
// warnings` would otherwise call dead code.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Shelf {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Belongs to the app in front.
    pub front: bool,
    /// Stable while the window lives, so the pet can tell that the
    /// window under it moved rather than that another one appeared.
    pub id: u64,
    /// The owning process, to find the window of a waiting session.
    pub pid: u32,
}

/// The app in front, by name only: enough to tell a meeting from a
/// music player, nothing about what is in it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct FrontApp {
    pub name: String,
    /// The bundle id on macOS, the executable's file name on Windows.
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Scene {
    /// Front to back, as the system stacks them.
    pub shelves: Vec<Shelf>,
    /// The app in front covers a whole display.
    pub fullscreen: bool,
    pub front_app: Option<FrontApp>,
}

/// `scale` turns the platform's window units into the pet window's
/// physical pixels; macOS reports points, Windows pixels.
pub fn scene(scale: f64) -> Scene {
    imp::scene(scale)
}

/// Windows shorter or narrower than this are tooltips, palettes and
/// helper surfaces, not something to stand on.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
const MIN_SIDE: f64 = 40.0;

#[cfg(target_os = "macos")]
mod imp {
    use super::{FrontApp, Scene, Shelf, MIN_SIDE};
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::{CGDisplay, CGRect};
    use core_graphics::window::{
        copy_window_info, kCGNullWindowID, kCGWindowListExcludeDesktopElements,
        kCGWindowListOptionOnScreenOnly,
    };
    use objc2_app_kit::NSWorkspace;

    type Dict = CFDictionary<CFString, CFType>;

    fn number(dict: &Dict, key: &str) -> Option<f64> {
        dict.find(CFString::new(key))?
            .downcast::<CFNumber>()?
            .to_f64()
    }

    fn front() -> Option<(f64, FrontApp)> {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let name = app
            .localizedName()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let id = app
            .bundleIdentifier()
            .map(|s| s.to_string())
            .unwrap_or_default();
        Some((app.processIdentifier() as f64, FrontApp { name, id }))
    }

    fn covers_a_display(displays: &[CGRect], x: f64, y: f64, w: f64, h: f64) -> bool {
        displays
            .iter()
            .any(|d| d.origin.x == x && d.origin.y == y && d.size.width == w && d.size.height == h)
    }

    pub fn scene(scale: f64) -> Scene {
        let mut scene = Scene::default();
        let Some(list) = copy_window_info(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        ) else {
            return scene;
        };
        let (front, front_app) = front().unzip();
        scene.front_app = front_app;
        let me = std::process::id() as f64;
        let displays: Vec<CGRect> = CGDisplay::active_displays()
            .unwrap_or_default()
            .into_iter()
            .map(|id| CGDisplay::new(id).bounds())
            .collect();
        for item in list.iter() {
            let dict: Dict = unsafe { CFDictionary::wrap_under_get_rule(*item as CFDictionaryRef) };
            // Layer 0 is ordinary windows; menus, the dock and overlays
            // sit on other layers.
            if number(&dict, "kCGWindowLayer") != Some(0.0) {
                continue;
            }
            let pid = number(&dict, "kCGWindowOwnerPID");
            if pid == Some(me) || number(&dict, "kCGWindowAlpha") == Some(0.0) {
                continue;
            }
            let Some(bounds) = dict
                .find(CFString::new("kCGWindowBounds"))
                .and_then(|b| b.downcast::<CFDictionary>())
            else {
                continue;
            };
            let bounds: Dict =
                unsafe { CFDictionary::wrap_under_get_rule(bounds.as_concrete_TypeRef()) };
            let (Some(x), Some(y), Some(w), Some(h)) = (
                number(&bounds, "X"),
                number(&bounds, "Y"),
                number(&bounds, "Width"),
                number(&bounds, "Height"),
            ) else {
                continue;
            };
            if w < MIN_SIDE || h < MIN_SIDE {
                continue;
            }
            let is_front = pid.is_some() && pid == front;
            if is_front && covers_a_display(&displays, x, y, w, h) {
                scene.fullscreen = true;
            }
            scene.shelves.push(Shelf {
                x: x * scale,
                y: y * scale,
                width: w * scale,
                height: h * scale,
                front: is_front,
                id: number(&dict, "kCGWindowNumber").unwrap_or(0.0) as u64,
                pid: pid.unwrap_or(0.0) as u32,
            });
        }
        scene
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{FrontApp, Scene, Shelf, MIN_SIDE};
    use windows::core::{BOOL, PWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    };
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowLongPtrW, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    };

    struct Walk {
        scene: Scene,
        front: HWND,
        me: u32,
    }

    fn frame(hwnd: HWND) -> Option<RECT> {
        let mut rect = RECT::default();
        unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut rect as *mut RECT as *mut core::ffi::c_void,
                std::mem::size_of::<RECT>() as u32,
            )
        }
        .ok()?;
        Some(rect)
    }

    fn cloaked(hwnd: HWND) -> bool {
        let mut value: u32 = 0;
        unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED,
                &mut value as *mut u32 as *mut core::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            )
        }
        .is_ok()
            && value != 0
    }

    fn covers_its_monitor(hwnd: HWND, r: &RECT) -> bool {
        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            return false;
        }
        let m = info.rcMonitor;
        r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let walk = unsafe { &mut *(lparam.0 as *mut Walk) };
        if !unsafe { IsWindowVisible(hwnd) }.as_bool()
            || unsafe { IsIconic(hwnd) }.as_bool()
            || cloaked(hwnd)
        {
            return BOOL(1);
        }
        let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return BOOL(1);
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == walk.me {
            return BOOL(1);
        }
        let Some(r) = frame(hwnd) else {
            return BOOL(1);
        };
        let (w, h) = ((r.right - r.left) as f64, (r.bottom - r.top) as f64);
        if w < MIN_SIDE || h < MIN_SIDE {
            return BOOL(1);
        }
        let front = hwnd == walk.front;
        if front && covers_its_monitor(hwnd, &r) {
            walk.scene.fullscreen = true;
        }
        walk.scene.shelves.push(Shelf {
            x: r.left as f64,
            y: r.top as f64,
            width: w,
            height: h,
            front,
            id: hwnd.0 as usize as u64,
            pid,
        });
        BOOL(1)
    }

    /// The executable's file name, `Zoom.exe`; Windows has no cheap
    /// display name, so it serves as both.
    fn exe_name(pid: u32) -> Option<String> {
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
        };
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(process) };
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().map(str::to_owned)
    }

    pub fn scene(_scale: f64) -> Scene {
        let mut walk = Walk {
            scene: Scene::default(),
            front: unsafe { GetForegroundWindow() },
            me: std::process::id(),
        };
        // EnumWindows walks top to bottom in z-order, which is the order
        // the scene promises.
        let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut walk as *mut Walk as isize)) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(walk.front, Some(&mut pid)) };
        walk.scene.front_app = exe_name(pid).map(|exe| FrontApp {
            name: exe.trim_end_matches(".exe").to_owned(),
            id: exe,
        });
        walk.scene
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use super::Scene;

    /// Wayland has no global window list; the pet lives on the work
    /// area's bottom edge there.
    pub fn scene(_scale: f64) -> Scene {
        Scene::default()
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod tests {
    use super::*;

    // Reads the real screen, so it is only informative: run with
    // `cargo test -p sidepaw-desktop -- --ignored --nocapture scene`.
    #[test]
    #[ignore]
    fn prints_the_scene() {
        let s = scene(1.0);
        println!("fullscreen: {}, front app: {:?}", s.fullscreen, s.front_app);
        for w in &s.shelves {
            println!("{:?}", w);
        }
        assert!(s
            .shelves
            .iter()
            .all(|w| w.width >= MIN_SIDE && w.height >= MIN_SIDE));
    }
}

/// Hides the pet while the app in front is full screen (a talk, a
/// film, a game) and brings it back after. Only undoes its own hide,
/// so the tray's show/hide is left alone.
pub fn spawn_fullscreen_watcher(window: tauri::WebviewWindow) {
    const POLL: std::time::Duration = std::time::Duration::from_secs(1);
    tauri::async_runtime::spawn(async move {
        let mut hidden_by_us = false;
        loop {
            tokio::time::sleep(POLL).await;
            let fullscreen = scene(1.0).fullscreen;
            if fullscreen && !hidden_by_us && window.is_visible().unwrap_or(false) {
                hidden_by_us = window.hide().is_ok();
            } else if !fullscreen && hidden_by_us {
                let _ = window.show();
                hidden_by_us = false;
            }
        }
    });
}
