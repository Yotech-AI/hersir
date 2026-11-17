//! The glass behind the islands (GA-292; design/islands/README.md → The glass): which blur the system gives the
//! see-through window. The UI asks once at start (src/lib/glass.ts) and asks for the effect itself through Tauri's window
//! API (`setEffects`, the capability's `core:window:allow-set-effects`); tauri.conf.json asks for it when the window is
//! made, so the glass is right from the first paint.

/// What blurs the desktop behind Hersir's window:
/// - `"vibrancy"` on macOS (`underWindowBackground`);
/// - `"mica"` on Windows 11 (build 22000 and later);
/// - `"none"` on an older Windows: it has no Mica (Tauri's call does nothing there), so the background stays plain;
/// - `"compositor"` on Hyprland (told apart by `HYPRLAND_INSTANCE_SIGNATURE`): Linux has no window effects, but Hyprland
///   blurs a see-through window by its own rules (`decoration:blur:enabled`, on by default; Hersir can't tell whether it
///   is on in yours);
/// - `"none"` on Linux other than Hyprland: KWin blurs only a window that asks for it and GNOME doesn't blur, so there
///   the background stays plain (design/islands/README.md → How it is built).
///
/// Async, like every command but GA-191's six, so it runs on the async runtime and not on the main thread (GA-299).
#[tauri::command]
pub async fn glass_support() -> &'static str {
    support()
}

pub fn support() -> &'static str {
    #[cfg(target_os = "macos")]
    return "vibrancy";
    #[cfg(windows)]
    return if windows_version::OsVersion::current().build >= 22000 { "mica" } else { "none" };
    #[cfg(not(any(target_os = "macos", windows)))]
    linux_support(std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").as_deref())
}

/// Linux's answer, from `HYPRLAND_INSTANCE_SIGNATURE`'s value.
pub fn linux_support(hyprland: Option<&std::ffi::OsStr>) -> &'static str {
    if hyprland.is_some_and(|s| !s.is_empty()) { "compositor" } else { "none" }
}
