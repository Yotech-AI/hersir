// GA-248: each GIZAI_ variable Hersir reads has a HERSIR_ name, which wins when both are set; the GIZAI_ name still
// works alone, and an empty one counts as unset. Here the ones of the update and the chat's helper: HERSIR_REPO,
// HERSIR_RELEASES_URL and HERSIR_MCP_BIN. The environment belongs to the whole process, so this file has a single test.
use gizai_agents::os::hersir_var;
use gizai_lib::mcp::shim_bin;
use gizai_lib::update::Source;

const NAMES: [&str; 6] = ["HERSIR_REPO", "GIZAI_REPO", "HERSIR_RELEASES_URL", "GIZAI_RELEASES_URL", "HERSIR_MCP_BIN", "GIZAI_MCP_BIN"];

/// Sets the variables in `vars` and unsets the others of NAMES.
fn with(vars: &[(&str, &str)]) {
    for name in NAMES {
        // SAFETY: the only test in this binary, and nothing else runs meanwhile.
        unsafe { std::env::remove_var(name) };
    }
    for (name, value) in vars {
        // SAFETY: as above
        unsafe { std::env::set_var(name, value) };
    }
}

#[test]
fn a_hersir_variable_wins_over_its_gizai_name_which_still_works_alone() {
    // the rule
    with(&[]);
    assert_eq!(hersir_var("REPO"), None);
    with(&[("GIZAI_REPO", "/srv/gizai.git")]);
    assert_eq!(hersir_var("REPO").as_deref(), Some("/srv/gizai.git".as_ref()), "the GIZAI_ name alone");
    with(&[("HERSIR_REPO", "/srv/hersir.git")]);
    assert_eq!(hersir_var("REPO").as_deref(), Some("/srv/hersir.git".as_ref()), "the HERSIR_ name alone");
    with(&[("HERSIR_REPO", "/srv/hersir.git"), ("GIZAI_REPO", "/srv/gizai.git")]);
    assert_eq!(hersir_var("REPO").as_deref(), Some("/srv/hersir.git".as_ref()), "both: HERSIR_ wins");
    with(&[("HERSIR_REPO", ""), ("GIZAI_REPO", "/srv/gizai.git")]);
    assert_eq!(hersir_var("REPO").as_deref(), Some("/srv/gizai.git".as_ref()), "an empty HERSIR_ one counts as unset");

    // where releases and their source come from (Settings → Updates)
    with(&[]);
    let own = Source::from_env();
    assert_eq!(own.repo, gizai_agents::update::REPO);
    with(&[("GIZAI_REPO", "/srv/gizai.git"), ("GIZAI_RELEASES_URL", "file:///gizai.json")]);
    assert_eq!(Source::from_env(), Source::new("/srv/gizai.git", Some("file:///gizai.json".into())), "the GIZAI_ names alone");
    with(&[("HERSIR_REPO", "/srv/hersir.git"), ("HERSIR_RELEASES_URL", "file:///hersir.json")]);
    assert_eq!(Source::from_env(), Source::new("/srv/hersir.git", Some("file:///hersir.json".into())), "the HERSIR_ names alone");
    with(&[("HERSIR_REPO", "/srv/hersir.git"), ("HERSIR_RELEASES_URL", "file:///hersir.json"),
           ("GIZAI_REPO", "/srv/gizai.git"), ("GIZAI_RELEASES_URL", "file:///gizai.json")]);
    assert_eq!(Source::from_env(), Source::new("/srv/hersir.git", Some("file:///hersir.json".into())), "both: HERSIR_ wins");
    with(&[("HERSIR_RELEASES_URL", "file:///hersir.json"), ("GIZAI_REPO", "/srv/gizai.git")]);
    assert_eq!(Source::from_env(), Source::new("/srv/gizai.git", Some("file:///hersir.json".into())), "each by itself");

    // the chat's helper, hersir-mcp
    let tmp = tempfile::tempdir().unwrap();
    let (hersir, gizai) = (tmp.path().join("hersir-mcp"), tmp.path().join("gizai-mcp"));
    std::fs::write(&hersir, "").unwrap();
    std::fs::write(&gizai, "").unwrap();
    let (h, g) = (hersir.to_str().unwrap(), gizai.to_str().unwrap());
    with(&[("GIZAI_MCP_BIN", g)]);
    assert_eq!(shim_bin(), Some(gizai.clone()), "GIZAI_MCP_BIN alone");
    with(&[("HERSIR_MCP_BIN", h)]);
    assert_eq!(shim_bin(), Some(hersir.clone()), "HERSIR_MCP_BIN alone");
    with(&[("HERSIR_MCP_BIN", h), ("GIZAI_MCP_BIN", g)]);
    assert_eq!(shim_bin(), Some(hersir.clone()), "both: HERSIR_MCP_BIN wins");
    with(&[]);
}
