//! Hersir's main window stays on Hersir's own pages (GA-362). A link out opens in your browser where Hersir shows it
//! (`openUrl`: the Markdown views, the Docs page's `Prose`); this guard is what holds when one isn't caught there, such
//! as a link in a project's changelog: the window refuses to go to a site, a file or any other address, and Hersir's log
//! says which site it refused.
use tauri::{Manager, Runtime, Url, Webview};

/// The window the guard keeps on Hersir's own pages.
const MAIN: &str = "main";

/// Whether the main window may go to `url`: one of Hersir's own pages. They are at tauri://localhost on Linux and
/// macOS and at http://tauri.localhost on Windows (https with Tauri's useHttpsScheme); a debug build loads them from
/// `dev`, the config's devUrl (http://localhost:1420), when given.
pub fn own_page(url: &Url, dev: Option<&Url>) -> bool {
    let tauri = match url.scheme() {
        "tauri" => url.host_str() == Some("localhost") && url.port().is_none(),
        "http" | "https" => url.host_str() == Some("tauri.localhost") && url.port().is_none(),
        _ => false,
    };
    let from_dev = dev.is_some_and(|d| {
        d.scheme() == url.scheme() && d.host_str() == url.host_str() && d.port_or_known_default() == url.port_or_known_default()
    });
    tauri || from_dev
}

/// The guard, as a plugin: every navigation of the main window is checked with `own_page`; other windows are left alone.
pub fn guard<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("gizai-navigation")
        .on_navigation(|webview: &Webview<R>, url: &Url| {
            if webview.label() != MAIN {
                return true;
            }
            // the dev server only for a dev build (cargo without the custom-protocol feature, `npm run tauri dev`)
            let dev = if tauri::is_dev() { webview.config().build.dev_url.as_ref() } else { None };
            let allowed = own_page(url, dev);
            if !allowed {
                eprintln!("gizai: Hersir's window stays on Hersir's own pages: it didn't go to {}", site(url));
            }
            allowed
        })
        .build()
}

/// The site of `url` for Hersir's log, not the whole address: an address can hold a key.
fn site(url: &Url) -> String {
    match url.host_str() {
        Some(host) => format!("{}://{host}{}", url.scheme(), url.port().map(|p| format!(":{p}")).unwrap_or_default()),
        None => format!("a {}: address", url.scheme()),
    }
}
