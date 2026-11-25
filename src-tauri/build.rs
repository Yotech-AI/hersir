use std::path::Path;

fn main() {
    // Linux (GA-222): the hersir binary exports its own gtk_clipboard_request_targets (src/paste_guard.rs), so WebKitGTK's
    // calls reach it rather than GTK's. Exported, it is also never dropped as unused when the binary is linked. The
    // target's system, not this build script's: cfg!(target_os) here would be the one building.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bin=hersir=-Wl,--export-dynamic-symbol=gtk_clipboard_request_targets");
    }
    // GA-343: the board's view for Claude Desktop (src/tools/apps.rs), from src/widget (`vite build --mode mcp-app`).
    built("mcp-app", "mcp-app.html", "Claude Desktop's board view", NOT_BUILT);
    // A project's Docs page for a browser (src/docs_server.rs), from src/docs (`vite build --mode docs-web`).
    built("docs-web", "docs.html", "the Docs page for a browser", DOCS_NOT_BUILT);
    tauri_build::build()
}

/// One HTML file that `npm run build` makes into `dir`, copied to OUT_DIR for `include_str!`. Without that build (cargo
/// alone), `stand_in`, a page that says so, and a warning. The folder is in git (its .gitignore keeps the file out), so
/// cargo sees the file come and change.
fn built(dir: &str, file: &str, what: &str, stand_in: &str) {
    println!("cargo:rerun-if-changed={dir}");
    let html = std::fs::read_to_string(Path::new(dir).join(file)).unwrap_or_else(|_| {
        println!("cargo:warning={dir}/{file} is missing: {what} is a stand-in page. npm run build makes it.");
        stand_in.to_string()
    });
    let out = Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join(file);
    std::fs::write(out, html).expect("OUT_DIR is writable");
}

/// The view when it wasn't built.
const NOT_BUILT: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Hersir board</title></head>\
<body><p>This Hersir was built without its board view for Claude Desktop: npm run build makes it.</p></body></html>\n";

/// The Docs page when it wasn't built.
const DOCS_NOT_BUILT: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Docs</title></head>\
<body><p>This Hersir was built without its Docs page for a browser: npm run build makes it.</p></body></html>\n";
