//! The fake Hersir Cloud signs in as the real one does, through the OAuth calls `gizai_agents::oauth` makes, without a
//! browser page: `/oauth/authorize` sends the browser straight back with a code (`fake::browse` plays the browser), and
//! the token it gives registers this computer with `POST /devices`.

use std::time::Duration;

use gizai_agents::oauth;
use gizai_cloud_client::api::Api;
use gizai_cloud_client::fake::{self, EMAIL, FakeCloud};
use gizai_cloud_client::Error;

use crate::common::{device_id, new_device};

/// Signs in to `fake` as the app does: discovery, the browser's round trip, and the code exchanged for tokens.
fn sign_in(fake: &FakeCloud) -> oauth::Saved {
    let d = oauth::discover(&format!("{}/api", fake.url()), None).expect("discovery");
    assert_eq!(d.issuer, fake.url());
    assert_eq!(d.authorization_endpoint, format!("{}/oauth/authorize", fake.url()));
    assert_eq!(d.token_endpoint, format!("{}/oauth/token", fake.url()));
    assert_eq!(d.scope.as_deref(), Some("device:register"));
    assert_eq!(d.service_name.as_deref(), Some("Hersir Cloud"));
    let pending = oauth::begin(&d, Some("hersir-desktop")).expect("begin");
    let url = pending.authorize_url.clone();
    let waiting = std::thread::spawn(move || oauth::finish(pending, Duration::from_secs(60)));
    let page = fake::browse(&url).expect("the browser's round trip");
    assert!(page.contains("signed in"), "{page}");
    waiting.join().unwrap().expect("the code exchanged for tokens")
}

#[test]
fn hersir_signs_in_to_the_fake_without_a_browser_page_and_registers_this_computer() {
    let fake = FakeCloud::start().unwrap();
    let saved = sign_in(&fake);
    assert!(saved.refresh_token.is_some());
    assert!(saved.expires_at.is_some());
    let id = device_id();
    let registered = Api::register(fake.url(), &saved.access_token, &new_device(&id, "Signed in")).unwrap();
    assert_eq!(registered.account.id, "1");
    assert_eq!(registered.account.email, EMAIL);
    assert!(registered.expires_at > 0);
    let api = Api::new(fake.url(), &registered.device_token).unwrap();
    let devices = api.devices().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].id, id);
    assert!(devices[0].this);
    // the browser's token registers once
    assert!(matches!(Api::register(fake.url(), &saved.access_token, &new_device(&device_id(), "Again")), Err(Error::SignedOut(_))));
}

#[test]
fn the_fake_signs_in_the_account_it_is_told_to() {
    let fake = FakeCloud::start().unwrap();
    let other = fake.add_account("second@example.com");
    fake.sign_in_as(&other.id);
    let saved = sign_in(&fake);
    let registered = Api::register(fake.url(), &saved.access_token, &new_device(&device_id(), "Theirs")).unwrap();
    assert_eq!(registered.account, other);
}

#[test]
fn a_refreshed_token_registers_too() {
    let fake = FakeCloud::start().unwrap();
    let saved = sign_in(&fake);
    let renewed = oauth::refresh(&saved).expect("refresh");
    assert_ne!(renewed.access_token, saved.access_token);
    Api::register(fake.url(), &renewed.access_token, &new_device(&device_id(), "Refreshed")).unwrap();
}

#[test]
fn the_fake_refuses_a_sign_in_that_isnt_pkce_s256_to_a_loopback_address() {
    let fake = FakeCloud::start().unwrap();
    let base = format!("{}/oauth/authorize?response_type=code&client_id=x&code_challenge=abc&code_challenge_method=S256&state=s", fake.url());
    let bad_redirect = fake::browse(&format!("{base}&redirect_uri=https%3A%2F%2Fevil.example%2Fcallback")).unwrap_err();
    assert!(bad_redirect.contains("400"), "{bad_redirect}");
    let plain = format!("{}/oauth/authorize?response_type=code&client_id=x&code_challenge=abc&code_challenge_method=plain&redirect_uri=http%3A%2F%2F127.0.0.1%3A1%2Fcallback", fake.url());
    assert!(fake::browse(&plain).unwrap_err().contains("400"));
}
