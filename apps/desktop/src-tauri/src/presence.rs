//! How long since the user last touched the keyboard, mouse or
//! trackpad anywhere, from the OS's own counter. Neither macOS nor
//! Windows asks for a permission for this: it is a number, not the
//! input itself. Linux has no portable call, so the pet falls back to
//! watching the cursor there.

/// Seconds since the last input event, or `None` where unknown.
pub fn idle_seconds() -> Option<f64> {
    imp::idle_seconds()
}

#[cfg(target_os = "macos")]
mod imp {
    // CGEventSourceStateID::CombinedSessionState and kCGAnyInputEventType.
    const COMBINED_SESSION_STATE: i32 = 0;
    const ANY_INPUT_EVENT: u32 = u32::MAX;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }

    pub fn idle_seconds() -> Option<f64> {
        // SAFETY: a pure query with no pointers; both arguments are
        // documented constants.
        let s = unsafe {
            CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT)
        };
        s.is_finite().then_some(s.max(0.0))
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    pub fn idle_seconds() -> Option<f64> {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // SAFETY: `info` is a valid, sized LASTINPUTINFO for the call.
        if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
            return None;
        }
        // Both are 32-bit tick counts; wrapping_sub survives the 49-day wrap.
        let ms = unsafe { GetTickCount() }.wrapping_sub(info.dwTime);
        Some(f64::from(ms) / 1000.0)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    pub fn idle_seconds() -> Option<f64> {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn the_os_reports_a_plausible_idle_time() {
        let s = super::idle_seconds().expect("an idle counter on this OS");
        assert!((0.0..86_400.0 * 365.0).contains(&s), "{s}");
    }
}
