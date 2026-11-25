// GA-359 QA: a debug build reads HERSIR_CLOUD_URL, HERSIR_CLOUD_CLIENT_ID and HERSIR_CLOUD_ENTITLEMENT_KEY, so it can
// point at GA-358's fake Hersir Cloud: signed in there, the key made and confirmed, Back up now puts the snapshot in the
// fake. A release build (this test built with --release) ignores them and keeps the production values. The environment
// belongs to the whole process, so this file has a single test.
use std::sync::Arc;
use std::time::{Duration, Instant};

use gizai_cloud_client::fake::{self, FakeCloud};
use gizai_lib::cloud::{self, Config};

#[test]
fn the_hersir_cloud_variables_point_a_debug_build_at_the_fake_and_a_release_build_ignores_them() {
    let f = FakeCloud::start().unwrap();
    // SAFETY: the only test in this binary, and nothing else runs meanwhile.
    unsafe {
        std::env::set_var("HERSIR_CLOUD_URL", format!("{}/", f.url()));
        std::env::set_var("HERSIR_CLOUD_CLIENT_ID", "qa-desktop-client");
        std::env::set_var("HERSIR_CLOUD_ENTITLEMENT_KEY", f.entitlement_key_b64());
    }
    let from_env = Config::from_env();
    if cfg!(debug_assertions) {
        assert_eq!(from_env.server, f.url());
        assert_eq!(from_env.client_id, "qa-desktop-client");
        assert_eq!(from_env.entitlement_key, Some(f.entitlement_key()));
    } else {
        assert_eq!(from_env, Config::production(), "a release build ignores HERSIR_CLOUD_*");
        assert_eq!(from_env.server, "https://cloud.hersir.dev");
        assert_eq!(from_env.client_id, "01a21388-1d0c-7e76-9253-3af81455f4d0");
        return;
    }

    // the debug build, as the app starts it: Hersir Cloud from the variables (test_state points it nowhere)
    let tmp = tempfile::tempdir().unwrap();
    let mut st = gizai_lib::test_state(tmp.path());
    st.cloud = Arc::new(cloud::Cloud::default());
    assert_eq!(st.cloud.config().server, f.url());
    let browser = |url: &str| -> Result<(), String> {
        let url = url.to_string();
        std::thread::spawn(move || {
            let _ = fake::browse(&url);
        });
        Ok(())
    };
    let s = cloud::sign_in(&st, browser, Duration::from_secs(60)).unwrap();
    assert!(s.signed_in);
    assert!(s.plan.is_some(), "the entitlement checks with the key from the variable");
    let code = cloud::make_key(&st).unwrap().recovery_code;
    cloud::confirm_recovery(&st, code.rsplit('-').next().unwrap()).unwrap();
    cloud::back_up_now(&st).unwrap();
    let end = Instant::now() + Duration::from_secs(120);
    while cloud::status(&st).backup.running.is_some() {
        assert!(Instant::now() < end, "the backup ended");
        std::thread::sleep(Duration::from_millis(20));
    }
    let snaps = f.snapshots(&f.account().id);
    assert_eq!(snaps.len(), 1, "the fake has the snapshot");
    assert_eq!(snaps[0].reason, "manual");
    assert!(snaps[0].device_id.eq_ignore_ascii_case(st.db.device_id()));
    assert_eq!(cloud::account_url(&st), format!("{}/account", f.url()));
}
