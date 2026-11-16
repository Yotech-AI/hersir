//! GA-281 QA: what Hersir's own log leaves out (`redact::hidden`): users send us that log, so a password, key or token in
//! git's or gh's words, a command's error or a project's setup command must not be in it.
use gizai_agents::redact::{first_line, hidden, LEFT_OUT};

#[test]
fn the_password_in_a_url_is_left_out_and_the_rest_of_the_address_stays() {
    assert_eq!(hidden("fatal: unable to access 'https://jefsev:hunter2@github.com/o/r.git/': 403"),
               "fatal: unable to access 'https://jefsev:***@github.com/o/r.git/': 403");
    assert_eq!(hidden("https://x-access-token:ghs_16C7e42F292c6912E7710c838347Ae178B4a@github.com/o/r"),
               "https://x-access-token:***@github.com/o/r");
    assert_eq!(hidden("postgres://app:s3cret@db:5432/app"), "postgres://app:***@db:5432/app");
}

#[test]
fn a_whole_login_over_http_and_https_is_a_token_and_left_out() {
    assert_eq!(hidden("git push https://ghp_abc@github.com/o/r HEAD"), "git push https://***@github.com/o/r HEAD");
    assert_eq!(hidden("HTTP://tok@example.com"), "HTTP://***@example.com");
}

#[test]
fn ssh_logins_and_addresses_without_a_login_stay() {
    for text in ["ssh://git@github.com/o/r.git", "git@github.com:Yotech-AI/hersir.git", "https://github.com/o/r/pull/12",
                 "see https://example.com/a?b=c#d", "mailto:me@example.com"] {
        assert_eq!(hidden(text), text);
    }
}

#[test]
fn known_tokens_keep_only_their_start() {
    let cases = [
        ("ghp_", "ghp_1234567890abcdefghijABCDEFGHIJ123456"),
        ("gho_", "gho_1234567890abcdefghij"),
        ("github_pat_", "github_pat_11ABCDEFG0123456789_abcdefghijklmnopqrstuvwxyz"),
        ("glpat-", "glpat-abcdefghij0123456789"),
        ("sk-", "sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789"),
        ("sk-", "sk-proj-abcdefghijklmnopqrstuvwxyz"),
        ("xoxb-", "xoxb-1234567890-abcdef"),
        ("ATATT", "ATATT3xFfGF0abcdefghijklmnopqrstuvwxyz"),
        ("AKIA", "AKIAIOSFODNN7EXAMPLE"),
    ];
    for (start, token) in cases {
        assert_eq!(hidden(&format!("used {token} here")), format!("used {start}{LEFT_OUT} here"), "{token}");
    }
    let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
    assert_eq!(hidden(&format!("token {jwt}")), "token eyJ***");
}

#[test]
fn short_words_that_only_start_like_a_token_stay() {
    for text in ["ghp_x", "sk-1", "the AKIA prefix", "risk-free", "task-runner", "eyJ.a.b"] {
        assert_eq!(hidden(text), text);
    }
}

#[test]
fn the_value_after_a_secrets_name_is_left_out() {
    assert_eq!(hidden("GITHUB_TOKEN=abc123 npm ci"), "GITHUB_TOKEN=*** npm ci");
    assert_eq!(hidden("DB_PASSWORD=hunter2 php artisan migrate"), "DB_PASSWORD=*** php artisan migrate");
    assert_eq!(hidden(r#"{"api_key": "abc123", "name": "x"}"#), r#"{"api_key": "***", "name": "x"}"#);
    assert_eq!(hidden("client_secret: abc123"), "client_secret: ***");
    assert_eq!(hidden("mysql --password hunter2 -u root"), "mysql --password *** -u root");
    assert_eq!(hidden("curl -H 'Authorization: Bearer abc.def-123'"), "curl -H 'Authorization: *** ***'");
    assert_eq!(hidden("Bearer abc.def"), "Bearer ***");
    assert_eq!(hidden("Basic dXNlcjpwYXNz"), "Basic ***");
}

#[test]
fn ordinary_words_and_line_breaks_stay() {
    let text = "gizai: after run 01abc: git push failed\nremote: Permission to o/r.git denied.\n\tsee the log";
    assert_eq!(hidden(text), text);
    assert_eq!(hidden(""), "");
    assert_eq!(hidden("ümlaut and émoji 🙂 stay"), "ümlaut and émoji 🙂 stay");
}

#[test]
fn first_line_is_the_first_line_with_something_in_it() {
    assert_eq!(first_line("\n   \n  setup failed (exit 1)  \nnpm ERR! token abc\n"), "setup failed (exit 1)");
    assert_eq!(first_line("one"), "one");
    assert_eq!(first_line(""), "");
    assert_eq!(first_line(" \n\t\n"), "");
}

/// GA-281 QA, round 1: a password's value is left out whole, whatever characters it holds, and so is a value after a
/// name ending in KEY. Before the fix `DB_PASSWORD=p@ss!w0rd` became `DB_PASSWORD=***@ss!w0rd`: only up to the first
/// character outside letters, digits and `_-.~+/` was hidden.
#[test]
fn a_secret_value_with_any_characters_in_it_is_left_out_whole() {
    let cases = [
        ("DB_PASSWORD=p@ss!w0rd npm ci", ["p@ss", "ss!w0rd"], "npm ci"),
        ("PASSWORD=\"ab#cd$ef\" make", ["ab#", "cd$ef"], "make"),
        ("api_key=abc%2Fdef php artisan", ["abc", "2Fdef"], "php artisan"),
        ("TOKEN=abc,def,ghi yarn", ["abc", "def,ghi"], "yarn"),
        ("mysql --password 'p@ss w0rd' -u root", ["p@ss", "w0rd"], "-u root"),
        ("OPENAI_KEY=abcdef0123456789abcdef node x.js", ["abcdef0123456789abcdef", "abcdef"], "node x.js"),
        ("APP_KEY=base64:Zm9vYmFyYmF6cXV4 php artisan", ["Zm9vYmFyYmF6cXV4", "base64:Zm9v"], "php artisan"),
    ];
    for (text, secrets, rest) in cases {
        let out = hidden(text);
        for secret in secrets {
            assert!(!out.contains(secret), "{text:?} -> {out:?} still holds {secret:?}");
        }
        assert!(out.ends_with(rest), "{text:?} -> {out:?} should keep {rest:?}");
        assert!(out.contains(LEFT_OUT), "{text:?} -> {out:?}");
    }
}
