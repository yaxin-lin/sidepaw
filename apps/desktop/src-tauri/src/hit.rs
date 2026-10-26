//! Per-pixel click-through. A fully click-through window cannot be
//! dragged, so the web view reports which cells of the window are
//! opaque and a poller flips `set_ignore_cursor_events` depending on
//! where the system cursor is. The web view gets no mouse events while
//! the window is ignoring them, which is why the cursor has to be read
//! from the OS side rather than from DOM events.

use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewWindow};

/// Coarse alpha mask of the window: one bit per `cell` by `cell`
/// logical pixels, `rows[y]` bit `x` set when that cell is opaque.
#[derive(Debug, Clone, Default)]
pub struct Mask {
    pub cell: u32,
    pub rows: Vec<u64>,
}

impl Mask {
    fn opaque_at(&self, x: f64, y: f64) -> bool {
        if self.cell == 0 || x < 0.0 || y < 0.0 {
            return false;
        }
        let cx = (x as u32) / self.cell;
        let cy = (y as u32) / self.cell;
        if cx >= 64 {
            return false;
        }
        self.rows
            .get(cy as usize)
            .map(|row| row & (1u64 << cx) != 0)
            .unwrap_or(false)
    }
}

#[derive(Default)]
pub struct HitMask(pub Mutex<Mask>);

pub const POLL: Duration = Duration::from_millis(40);

/// Decide for the current cursor position. Returns the new
/// ignore-cursor-events value, or `None` when it cannot be determined
/// (cursor API unavailable, window hidden) and nothing should change.
pub fn should_ignore(app: &AppHandle, window: &WebviewWindow, mask: &Mask) -> Option<bool> {
    let cursor = app.cursor_position().ok()?;
    let origin = window.outer_position().ok()?;
    let size = window.inner_size().ok()?;
    let scale = window.scale_factor().ok()?;
    let x = cursor.x - origin.x as f64;
    let y = cursor.y - origin.y as f64;
    if x < 0.0 || y < 0.0 || x >= size.width as f64 || y >= size.height as f64 {
        // Outside the window entirely: ignoring costs nothing and means
        // the first re-entry is already in the right mode.
        return Some(true);
    }
    Some(!mask.opaque_at(x / scale, y / scale))
}

pub fn spawn_poller(app: AppHandle, window: WebviewWindow) {
    tauri::async_runtime::spawn(async move {
        let mut ignoring: Option<bool> = None;
        loop {
            tokio::time::sleep(POLL).await;
            let mask = app.state::<HitMask>().0.lock().unwrap().clone();
            let Some(next) = should_ignore(&app, &window, &mask) else {
                continue;
            };
            if ignoring != Some(next) && window.set_ignore_cursor_events(next).is_ok() {
                ignoring = Some(next);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_lookup_uses_cells() {
        let mask = Mask {
            cell: 4,
            rows: vec![0, 0b0110],
        };
        assert!(!mask.opaque_at(0.0, 0.0));
        assert!(mask.opaque_at(5.0, 5.0));
        assert!(mask.opaque_at(11.9, 7.9));
        assert!(!mask.opaque_at(12.0, 5.0));
        assert!(!mask.opaque_at(5.0, 8.0));
    }

    #[test]
    fn empty_mask_is_transparent() {
        assert!(!Mask::default().opaque_at(1.0, 1.0));
    }
}
