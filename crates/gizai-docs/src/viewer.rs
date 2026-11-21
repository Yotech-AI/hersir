//! The docs page itself: yodocs' viewer (viewer/, whose README.md says what Hersir changed), which draws the Yodocs
//! Figma from a project's index (`index.rs`). Built into Hersir and served by path, in the app and in a browser.

macro_rules! files {
    ($($path:literal => $mime:literal),* $(,)?) => { &[$(($path, include_bytes!(concat!("../viewer/", $path)), $mime)),*] };
}

/// Every file of the page, with its media type. A test checks that none in viewer/ is missing.
pub const FILES: &[(&str, &[u8], &str)] = files! {
    "index.html" => "text/html; charset=utf-8",
    "fonts.css" => "text/css; charset=utf-8",
    "tokens.css" => "text/css; charset=utf-8",
    "app.css" => "text/css; charset=utf-8",
    "js/main.js" => "text/javascript; charset=utf-8",
    "js/reference.js" => "text/javascript; charset=utf-8",
    "js/search.js" => "text/javascript; charset=utf-8",
    "js/sections.js" => "text/javascript; charset=utf-8",
    "js/sidebar.js" => "text/javascript; charset=utf-8",
    "js/state.js" => "text/javascript; charset=utf-8",
    "js/theme.js" => "text/javascript; charset=utf-8",
    "js/ui.js" => "text/javascript; charset=utf-8",
    "vendor/minisearch.js" => "text/javascript; charset=utf-8",
    "fonts/inter-latin-400-normal.woff2" => "font/woff2",
    "fonts/inter-latin-500-normal.woff2" => "font/woff2",
    "fonts/inter-latin-600-normal.woff2" => "font/woff2",
    "fonts/inter-latin-ext-400-normal.woff2" => "font/woff2",
    "fonts/inter-latin-ext-500-normal.woff2" => "font/woff2",
    "fonts/inter-latin-ext-600-normal.woff2" => "font/woff2",
    "fonts/ibm-plex-mono-latin-400-normal.woff2" => "font/woff2",
    "fonts/ibm-plex-mono-latin-500-normal.woff2" => "font/woff2",
    "fonts/ibm-plex-mono-latin-ext-400-normal.woff2" => "font/woff2",
    "fonts/ibm-plex-mono-latin-ext-500-normal.woff2" => "font/woff2",
};

/// One of the page's files (`index.html`, `js/main.js`…): its bytes and media type.
pub fn file(path: &str) -> Option<(&'static [u8], &'static str)> {
    FILES.iter().find(|(p, _, _)| *p == path).map(|(_, bytes, mime)| (*bytes, *mime))
}
