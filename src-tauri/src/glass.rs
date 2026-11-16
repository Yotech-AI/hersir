//! The glass behind the islands (GA-292; design/islands/README.md → The glass): which blur the system gives the
//! see-through window. The UI asks once at start (src/lib/glass.ts) and asks for the effect itself through Tauri's window
//! API (`setEffects`, the capability's `core:window:allow-set-effects`); tauri.conf.json asks for it when the window is
//! made, so the glass is right from the first paint.

/// What blurs the desktop behind Hersir's window:
/// - `"vibrancy"` on macOS (`underWindowBackground`);
/// - `"mica"` on Windows 11 (build 22000 and later);
/// - `"none"` on an older Windows: it has no Mica (Tauri's call does nothing there), so the background stays plain;
/// - `"compositor"` on Linux: no window effects there; the desktop's compositor blurs a see-through window by its own
///   rules (Hyprland does by default, GNOME doesn't), and Hersir can't tell whether it does.
#[tauri::command]
pub fn glass_support() -> &'static str {
    support()
}

pub fn support() -> &'static str {
    #[cfg(target_os = "macos")]
    return "vibrancy";
    #[cfg(windows)]
    return if windows_version::OsVersion::current().build >= 22000 { "mica" } else { "none" };
    #[cfg(not(any(target_os = "macos", windows)))]
    "compositor"
}
