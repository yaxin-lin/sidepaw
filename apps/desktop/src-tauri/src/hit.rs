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

/// The mask the web view last sent, and the web view's zoom (the pet
/// size), which the mask's CSS pixels have to be scaled by to meet the
/// window's logical pixels.
pub struct HitMask {
    pub mask: Mutex<Mask>,
    pub zoom: Mutex<f64>,
}

impl Default for HitMask {
    fn default() -> Self {
        HitMask {
            mask: Mutex::default(),
            zoom: Mutex::new(1.0),
        }
    }
}

/// The cursor as the web view would like to see it: relative to the
/// window's origin, in logical pixels, with the window's logical size.
/// Negative or beyond the size means outside.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CursorRelative {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn cursor_relative(app: &AppHandle, window: &WebviewWindow) -> Option<CursorRelative> {
    let cursor = app.cursor_position().ok()?;
    let origin = window.outer_position().ok()?;
    let size = window.inner_size().ok()?;
    let scale = window.scale_factor().ok()?;
    Some(CursorRelative {
        x: (cursor.x - origin.x as f64) / scale,
        y: (cursor.y - origin.y as f64) / scale,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
    })
}

pub const POLL: Duration = Duration::from_millis(40);

/// Decide for the current cursor position. Returns the new
/// ignore-cursor-events value, or `None` when it cannot be determined
/// (cursor API unavailable, window hidden) and nothing should change.
pub fn should_ignore(
    app: &AppHandle,
    window: &WebviewWindow,
    mask: &Mask,
    zoom: f64,
) -> Option<bool> {
    let c = cursor_relative(app, window)?;
    if c.x < 0.0 || c.y < 0.0 || c.x >= c.width || c.y >= c.height {
        // Outside the window entirely: ignoring costs nothing and means
        // the first re-entry is already in the right mode.
        return Some(true);
    }
    Some(!mask.opaque_at(c.x / zoom, c.y / zoom))
}

pub fn spawn_poller(app: AppHandle, window: WebviewWindow) {
    tauri::async_runtime::spawn(async move {
        let mut ignoring: Option<bool> = None;
        loop {
            tokio::time::sleep(POLL).await;
            let state = app.state::<HitMask>();
            let mask = state.mask.lock().unwrap().clone();
            let zoom = *state.zoom.lock().unwrap();
            let Some(next) = should_ignore(&app, &window, &mask, zoom) else {
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
