//! The crate's integration tests as one test program (GA-358, as GA-213 has it for the other crates): every file in
//! this folder is a module, declared below. They run against the fake Hersir Cloud (`gizai_cloud_client::fake`), each
//! test with a fake of its own on 127.0.0.1, so they run side by side. Paths in `include_str!` are relative to this
//! folder.

mod common;

mod api_errors_test;
mod api_test;
mod backup_restore_test;
mod chunks_test;
mod docs_test;
mod entitlement_test;
mod keys_test;
mod no_plaintext_test;
mod sign_in_test;
