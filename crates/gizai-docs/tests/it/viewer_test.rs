//! The docs page's own files (yodocs' viewer, viewer/): every one is built in and served by path with its media type,
//! and Hersir's changes to it hold: the Figma's fonts, search built in the page, the app's theme first.
use gizai_docs::viewer::{file, FILES};

fn text(path: &str) -> String {
    String::from_utf8(file(path).unwrap_or_else(|| panic!("{path} isn't served")).0.to_vec()).unwrap()
}

#[test]
fn every_file_in_the_viewer_folder_is_served() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("viewer");
    let mut on_disk = vec![];
    let mut dirs = vec![root.clone()];
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() { dirs.push(p); continue; }
            let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
            // the licences and the README are for people reading the repository, not for the page
            if !rel.ends_with(".txt") && rel != "README.md" { on_disk.push(rel); }
        }
    }
    on_disk.sort();
    let mut served: Vec<String> = FILES.iter().map(|(p, _, _)| p.to_string()).collect();
    served.sort();
    assert_eq!(served, on_disk);
    assert_eq!(file("js/main.js").unwrap().1, "text/javascript; charset=utf-8");
    assert_eq!(file("fonts/inter-latin-400-normal.woff2").unwrap().1, "font/woff2");
    assert!(file("../Cargo.toml").is_none() && file("README.md").is_none());
}

#[test]
fn hersirs_changes_hold() {
    assert!(text("index.html").contains(r#"<link rel="stylesheet" href="fonts.css" />"#));
    let fonts = text("fonts.css");
    assert_eq!(fonts.matches("@font-face").count(), 10);
    assert!(fonts.contains("font-family: 'Inter'") && fonts.contains("font-family: 'IBM Plex Mono'"));
    for url in fonts.split("url(").skip(1).map(|s| s.split(')').next().unwrap()) {
        assert!(file(url).is_some(), "fonts.css names {url}, which isn't served");
    }
    let search = text("js/search.js");
    assert!(!search.contains("fetch('search.json')") && search.contains("mini.addAll(documents(state.index))"));
    assert!(text("js/theme.js").contains("get('theme')"));
    assert!(!text("js/sidebar.js").contains("search.json"));
}
