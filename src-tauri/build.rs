use std::path::Path;

fn main() {
    // Linux (GA-222): the hersir binary exports its own gtk_clipboard_request_targets (src/paste_guard.rs), so WebKitGTK's
    // calls reach it rather than GTK's. Exported, it is also never dropped as unused when the binary is linked. The
    // target's system, not this build script's: cfg!(target_os) here would be the one building.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bin=hersir=-Wl,--export-dynamic-symbol=gtk_clipboard_request_targets");
    }
    mcp_app();
    tauri_build::build()
}

/// GA-343: the board's view for Claude Desktop (src/tools/apps.rs), one HTML file that `npm run build` makes from
/// src/widget into mcp-app/ (`vite build --mode mcp-app`), copied to OUT_DIR for `include_str!`. Without that build
/// (cargo alone), a page that says so, and a warning. The folder is in git (its .gitignore keeps the file out), so cargo
/// sees the file come and change.
fn mcp_app() {
    println!("cargo:rerun-if-changed=mcp-app");
    let built = Path::new("mcp-app").join("mcp-app.html");
    let html = std::fs::read_to_string(&built).unwrap_or_else(|_| {
        println!("cargo:warning=mcp-app/mcp-app.html is missing: Claude Desktop's board view is a stand-in page. npm run build makes it.");
        NOT_BUILT.to_string()
    });
    let out = Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("mcp-app.html");
    std::fs::write(out, html).expect("OUT_DIR is writable");
}

/// The view when it wasn't built.
const NOT_BUILT: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Hersir board</title></head>\
<body><p>This Hersir was built without its board view for Claude Desktop: npm run build makes it.</p></body></html>\n";
