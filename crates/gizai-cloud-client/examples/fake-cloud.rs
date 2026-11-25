//! A fake Hersir Cloud on 127.0.0.1 for local runs: `cargo run -p gizai-cloud-client --example fake-cloud [port]`.
//! It prints its address and its entitlement key, and runs until it is stopped (Ctrl+C). Everything is in memory.
//!
//! - The port: the first argument, else `HERSIR_FAKE_CLOUD_PORT`, else any free one.
//! - The entitlement key: random, unless `HERSIR_FAKE_CLOUD_SEED` holds the base64 of a 32-byte Ed25519 seed, so the
//!   key stays the same from one run to the next.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use gizai_cloud_client::fake::{FakeCloud, Options};

fn main() {
    let port = std::env::args().nth(1).or_else(|| std::env::var("HERSIR_FAKE_CLOUD_PORT").ok())
        .map(|p| p.trim().parse::<u16>().unwrap_or_else(|_| fail(&format!("\"{p}\" isn't a port"))))
        .unwrap_or(0);
    let entitlement_seed = std::env::var("HERSIR_FAKE_CLOUD_SEED").ok().map(|s| {
        STANDARD.decode(s.trim()).ok().and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
            .unwrap_or_else(|| fail("HERSIR_FAKE_CLOUD_SEED isn't the base64 of 32 bytes"))
    });
    let fake = FakeCloud::start_with(Options { port, entitlement_seed }).unwrap_or_else(|e| fail(&format!("It couldn't start: {e}")));
    let account = fake.account();
    println!("Fake Hersir Cloud at {}", fake.url());
    println!("Entitlement key: {}", fake.entitlement_key_b64());
    println!("Account {} ({}), with an active plan. Any client id signs in, straight away.", account.id, account.email);
    println!("Stop it with Ctrl+C.");
    loop {
        std::thread::park();
    }
}

fn fail(why: &str) -> ! {
    eprintln!("fake-cloud: {why}");
    std::process::exit(2);
}
