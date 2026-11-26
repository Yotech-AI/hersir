//! GA-362 QA: Hersir's main window stays on Hersir's own pages (`navigation::own_page`, the guard's check): tauri://localhost
//! on Linux and macOS, http(s)://tauri.localhost on Windows, and the dev server only when a dev build gives it. A site,
//! the docs server, a file, a script, a look-alike host or another port is refused.
use gizai_lib::navigation::own_page;
use tauri::Url;

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

#[test]
fn hersirs_own_pages_on_each_system_are_allowed_with_any_route() {
    for own in ["tauri://localhost", "tauri://localhost/", "tauri://localhost/index.html#/devdocs/p1/changelog", "tauri://localhost/#/board",
                "http://tauri.localhost/", "http://tauri.localhost/#/devdocs/p1/", "https://tauri.localhost/index.html?x=1#/chat"] {
        assert!(own_page(&url(own), None), "{own}");
    }
}

#[test]
fn a_site_the_docs_server_a_file_a_script_or_a_look_alike_is_refused() {
    for elsewhere in [
        "https://example.com/", "http://example.com/changelog", "https://keepachangelog.com/en/1.1.0/",
        "http://127.0.0.1:4380/t/0123/p/p1/", "http://localhost:4380/", "http://localhost/", "http://localhost:1420/",
        "file:///etc/passwd", "data:text/html,<script>alert(1)</script>", "javascript:alert(1)", "about:blank", "ftp://tauri.localhost/",
        "tauri://localhost:1420/", "tauri://evil.example/", "tauri://localhost.evil.example/", "http://tauri.localhost:8080/",
        "http://tauri.localhost.evil.example/", "http://evil.example/tauri.localhost", "http://tauri.localhost@evil.example/",
        "https://evil.tauri.localhost/", "mailto:a@example.com", "blob:https://example.com/0f1e",
    ] {
        assert!(!own_page(&url(elsewhere), None), "{elsewhere}");
    }
}

#[test]
fn the_dev_server_only_when_a_dev_build_gives_it_and_only_at_its_own_port() {
    let dev = url("http://localhost:1420");
    assert!(own_page(&url("http://localhost:1420/"), Some(&dev)));
    assert!(own_page(&url("http://localhost:1420/#/devdocs/p1/changelog"), Some(&dev)));
    assert!(own_page(&url("tauri://localhost/"), Some(&dev)), "Hersir's own pages stay allowed");
    for other in ["http://localhost:1421/", "http://localhost/", "https://localhost:1420/", "http://127.0.0.1:1420/", "http://localhost:4380/t/0123/"] {
        assert!(!own_page(&url(other), Some(&dev)), "{other}");
    }
    assert!(!own_page(&url("http://localhost:1420/"), None), "not in a release build");
}
