//! The pet's size, do-not-disturb and mini mode: saved in `[pet]`,
//! changed from the tray, applied here and announced to the web views
//! with a `prefs-changed` event. Size is applied on this side because
//! it is the window and the web view zoom together; DND and mini mode
//! are behaviour the web view carries out.

use crate::hit::HitMask;
use sidepaw_daemon::config::{PetPrefs, PetSize};
use sidepaw_daemon::server::Running;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition};

pub const CHANGED: &str = "prefs-changed";

/// The pet window's size at `PetSize::M`, as in tauri.conf.json.
const BASE: (f64, f64) = (192.0, 300.0);

/// Resize the window around its bottom edge, so the pet keeps standing
/// where it stood, and zoom the web view to match.
pub fn apply_size(app: &AppHandle, size: PetSize) {
    let Some(window) = app.get_webview_window("pet") else {
        return;
    };
    let zoom = size.zoom();
    let before = window.outer_position().ok().zip(window.outer_size().ok());
    let _ = window.set_size(LogicalSize::new(BASE.0 * zoom, BASE.1 * zoom));
    let _ = window.set_zoom(zoom);
    *app.state::<HitMask>().zoom.lock().unwrap() = zoom;
    if let (Some((pos, old)), Ok(new)) = (before, window.outer_size()) {
        let y = pos.y + old.height as i32 - new.height as i32;
        let x = pos.x + (old.width as i32 - new.width as i32) / 2;
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

/// Save a change, apply what belongs to this side, and tell the windows.
pub fn change(app: &AppHandle, edit: impl FnOnce(&mut PetPrefs)) {
    let running = app.state::<Running>();
    let mut prefs = running.shared.pet_prefs();
    let before = prefs;
    edit(&mut prefs);
    if prefs == before {
        return;
    }
    if let Err(e) = running.shared.set_pet_prefs(prefs) {
        tracing::warn!("save pet prefs: {e:#}");
    }
    if prefs.size != before.size {
        apply_size(app, prefs.size);
    }
    let _ = app.emit(CHANGED, prefs);
}
