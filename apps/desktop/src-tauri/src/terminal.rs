//! Jump to the terminal a session runs in. The daemon knows the host
//! process and its tty (`origin`); here that becomes a window in
//! front: the exact tab in Terminal.app or iTerm2 on macOS, otherwise
//! the nearest ancestor process that owns a window (VS Code, Warp,
//! Windows Terminal, ...).

use sidepaw_daemon::origin::Origin;

pub fn focus(origin: &Origin) -> Result<(), String> {
    imp::focus(origin)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
const NO_WINDOW: &str = "no window found for the session's terminal";

#[cfg(target_os = "macos")]
fn ancestors(pid: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut cur = pid;
    for _ in 0..12 {
        let Ok(o) = std::process::Command::new("ps")
            .args(["-o", "ppid=", "-p", &cur.to_string()])
            .output()
        else {
            break;
        };
        let Ok(parent) = String::from_utf8_lossy(&o.stdout).trim().parse::<u32>() else {
            break;
        };
        if parent <= 1 {
            break;
        }
        out.push(parent);
        cur = parent;
    }
    out
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{ancestors, NO_WINDOW};
    use objc2_app_kit::{
        NSApplicationActivationOptions, NSApplicationActivationPolicy, NSRunningApplication,
    };
    use sidepaw_daemon::origin::Origin;
    use std::process::Command;

    fn running(app: &str) -> bool {
        Command::new("pgrep")
            .args(["-xq", app])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    // Returns the script's last value; "true" when it selected a tab.
    fn osascript(script: &str) -> Result<String, String> {
        let out = Command::new("osascript")
            .args(["-e", script])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }

    fn iterm_script(dev: &str) -> String {
        format!(
            r#"tell application "iTerm2"
  repeat with w in windows
    repeat with t in tabs of w
      repeat with s in sessions of t
        if tty of s is "{dev}" then
          tell w to select t
          tell t to select s
          set index of w to 1
          activate
          return "true"
        end if
      end repeat
    end repeat
  end repeat
end tell
return "false""#
        )
    }

    fn terminal_script(dev: &str) -> String {
        format!(
            r#"tell application "Terminal"
  repeat with w in windows
    repeat with t in tabs of w
      if tty of t is "{dev}" then
        set selected tab of w to t
        set index of w to 1
        activate
        return "true"
      end if
    end repeat
  end repeat
end tell
return "false""#
        )
    }

    // The first process up the chain that is a regular app (has a
    // Dock icon) is the window the user is looking for.
    fn activate_ancestor_app(pid: u32) -> Result<(), String> {
        for p in std::iter::once(pid).chain(ancestors(pid)) {
            let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(p as i32)
            else {
                continue;
            };
            if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
                continue;
            }
            #[allow(deprecated)]
            let ok =
                app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
            return if ok {
                Ok(())
            } else {
                Err("the app refused activation".into())
            };
        }
        Err(NO_WINDOW.into())
    }

    pub fn focus(origin: &Origin) -> Result<(), String> {
        if let Some(tty) = &origin.tty {
            let dev = format!("/dev/{tty}");
            // Only ask apps that are running; `tell application` would
            // launch one otherwise.
            if running("iTerm2") && osascript(&iterm_script(&dev))? == "true" {
                return Ok(());
            }
            if running("Terminal") && osascript(&terminal_script(&dev))? == "true" {
                return Ok(());
            }
        }
        activate_ancestor_app(origin.pid)
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::NO_WINDOW;
    use sidepaw_daemon::origin::Origin;
    use std::collections::HashMap;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow,
        ShowWindow, SW_RESTORE,
    };

    fn parents() -> HashMap<u32, u32> {
        let mut map = HashMap::new();
        let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return map;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if unsafe { Process32FirstW(snap, &mut entry) }.is_ok() {
            loop {
                map.insert(entry.th32ProcessID, entry.th32ParentProcessID);
                if unsafe { Process32NextW(snap, &mut entry) }.is_err() {
                    break;
                }
            }
        }
        let _ = unsafe { CloseHandle(snap) };
        map
    }

    struct Find {
        /// The session's process first, then its ancestors.
        pids: Vec<u32>,
        /// Index into `pids` and the window; lower index wins.
        best: Option<(usize, HWND)>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let find = unsafe { &mut *(lparam.0 as *mut Find) };
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return BOOL(1);
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if let Some(i) = find.pids.iter().position(|p| *p == pid) {
            // `Option::is_none_or` would need Rust 1.82; the MSRV is 1.80.
            if !matches!(find.best, Some((best, _)) if best <= i) {
                find.best = Some((i, hwnd));
            }
        }
        BOOL(1)
    }

    pub fn focus(origin: &Origin) -> Result<(), String> {
        let parents = parents();
        let mut pids = vec![origin.pid];
        let mut cur = origin.pid;
        for _ in 0..12 {
            match parents.get(&cur) {
                Some(&p) if p > 0 && p != cur => {
                    pids.push(p);
                    cur = p;
                }
                _ => break,
            }
        }
        let mut find = Find { pids, best: None };
        let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut find as *mut Find as isize)) };
        let Some((_, hwnd)) = find.best else {
            return Err(NO_WINDOW.into());
        };
        if unsafe { IsIconic(hwnd) }.as_bool() {
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
        }
        if unsafe { SetForegroundWindow(hwnd) }.as_bool() {
            Ok(())
        } else {
            Err("the window refused to come to the front".into())
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use sidepaw_daemon::origin::Origin;

    pub fn focus(_origin: &Origin) -> Result<(), String> {
        Err("jumping to the terminal is not supported on Linux yet".into())
    }
}
