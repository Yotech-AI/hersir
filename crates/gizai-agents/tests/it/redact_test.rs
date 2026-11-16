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

/// Each case: the text, the secrets that must be gone from it, and what must stay.
fn assert_left_out(cases: &[(&str, &[&str], &[&str])]) {
    for (text, secrets, kept) in cases {
        let out = hidden(text);
        for secret in *secrets {
            assert!(!out.contains(secret), "{text:?} -> {out:?} still holds {secret:?}");
        }
        for k in *kept {
            assert!(out.contains(k), "{text:?} -> {out:?} lost {k:?}");
        }
        assert!(out.contains(LEFT_OUT), "{text:?} -> {out:?}");
    }
}

/// GA-281 QA, round 2: secrets as they come in a project's setup command, an `.env` file, a header or a config file,
/// each left out whole with the command around it kept.
#[test]
fn secrets_in_setup_commands_headers_and_config_files_are_left_out_whole() {
    assert_left_out(&[
        ("PGPASSWORD=p@ss!w0rd psql -h db", &["p@ss", "w0rd"], &["PGPASSWORD=", "psql -h db"]),
        ("export AWS_SECRET_ACCESS_KEY=wJalr/K7MDENG+bPx=RfiCY && make", &["wJalr", "RfiCY", "K7MD"], &["&& make"]),
        ("npm config set //registry.npmjs.org/:_authToken=npm_a1b2c3d4 && npm ci", &["a1b2c3d4"], &["&& npm ci"]),
        ("mytool --token=abc$def&ghi run", &["abc", "def", "ghi"], &["--token=", " run"]),
        ("mytool --api-key 'p@ss w0rd' run", &["p@ss", "w0rd"], &["--api-key", " run"]),
        ("curl -H \"X-Api-Key: k3y!v@lue\" https://api.example.com", &["k3y", "v@lue"], &["https://api.example.com"]),
        ("curl -H 'PRIVATE-TOKEN: abc#123' https://gitlab.example.com", &["abc#123", "abc"], &["https://gitlab.example.com"]),
        ("git -c http.extraHeader=\"Authorization: Basic dXNlcjpw\" clone x", &["dXNlcjpw"], &["clone x"]),
        ("curl -u me:p@ss!word https://x.example.com", &["p@ss", "word"], &["-u me:", "https://x.example.com"]),
        ("curl --user=me:p@ss https://x.example.com", &["p@ss"], &["https://x.example.com"]),
        ("mysql -uroot -pS3cr3t! app < dump.sql", &["S3cr3t"], &["-uroot", "app < dump.sql"]),
        ("sh -c \"TOKEN=a\\\"b c npm ci\"", &["a\\\"b"], &["npm ci"]),
        ("PASSWORD=pässwörd€! npm ci", &["pässwörd", "€!"], &["npm ci"]),
        ("COMPOSER_AUTH='{\"github-oauth\": {\"github.com\": \"abc123\"}}' composer install", &["abc123"], &["composer install"]),
        ("COMPOSER_AUTH={\"http-basic\":{\"repo\":{\"username\":\"u\",\"password\":\"p w\"}}} composer install",
         &["\"p w\"", "p w"], &["composer install"]),
        ("['password' => 'p@ss w0rd', 'host' => 'db']", &["p@ss", "w0rd"], &["['password' => '", "'host' => 'db']"]),
        ("password := \"x y z\"", &["x y z"], &["password := \""]),
    ]);
}

#[test]
fn an_env_file_keeps_its_other_lines() {
    let env = "APP_NAME=Hersir\nAPP_KEY=base64:Zm9v/YmFy+YmF6==\nDB_PASSWORD='p@ss w0rd'\nDB_HOST=127.0.0.1\nMAIL_PASSWORD=\"m#il\"\n";
    let out = hidden(env);
    for secret in ["Zm9v", "YmF6", "p@ss", "w0rd", "m#il"] {
        assert!(!out.contains(secret), "{out:?} still holds {secret:?}");
    }
    for kept in ["APP_NAME=Hersir\n", "\nDB_HOST=127.0.0.1\n", "APP_KEY=", "DB_PASSWORD=", "MAIL_PASSWORD="] {
        assert!(out.contains(kept), "{out:?} lost {kept:?}");
    }
    assert_eq!(out.lines().count(), 5, "{out:?}");
}

#[test]
fn yaml_ini_and_header_values_run_to_the_lines_end_and_the_next_line_stays() {
    assert_eq!(hidden("client_secret: p@ss w0rd\nname: hersir"), "client_secret: *** ***\nname: hersir");
    assert_eq!(hidden("password = hunter 2\n[section]"), "password = *** ***\n[section]");
    assert_eq!(hidden("Authorization: token ghp_x\nAccept: */*"), "Authorization: *** ***\nAccept: */*");
}

/// What the developer named as staying: these are no secrets.
#[test]
fn words_like_a_secrets_name_without_a_value_stay() {
    for text in ["php artisan key:generate", "ssh-keygen -t ed25519", "the token expired, sign in again",
                 "remote: Invalid username or password.\nfatal: Authentication failed for 'https://github.com/o/r.git/'",
                 "echo done", "git push origin HEAD"] {
        assert_eq!(hidden(text), text);
    }
}

/// `hidden` runs on every background error Hersir logs, so it must never panic or hang, whatever text it gets: a
/// panic there would take Hersir down. Deterministic pseudo-random texts built from the pieces its rules look at.
#[test]
fn hidden_never_panics_or_hangs_on_odd_text() {
    let pieces = ["TOKEN", "password", "APP_KEY", "-p", "-u", "--user", "--token", "Bearer", "Basic", "Authorization",
                  "mysql", "=", "==", "=>", ":=", ":", ": ", " ", "\t", "\n", "\r\n", "'", "\"", "`", "\\", "{", "}",
                  "[", "]", "@", "://", "https://", "me:pw@", "ghp_", "eyJ", "é", "€", "🙂", "ü", "a", "1", ".", "/",
                  "x-access-token:", "sk-", "$", "#"];
    let mut seed: u64 = 0x2026_1116;
    let started = std::time::Instant::now();
    for _ in 0..20_000 {
        let mut text = String::new();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let len = (seed >> 33) % 24;
        for _ in 0..len {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            text.push_str(pieces[((seed >> 33) % pieces.len() as u64) as usize]);
        }
        let out = std::panic::catch_unwind(|| hidden(&text)).unwrap_or_else(|_| panic!("hidden panicked on {text:?}"));
        assert!(out.len() <= text.len() + 3 * text.len(), "{text:?} -> {out:?}");
    }
    assert!(started.elapsed().as_secs() < 30, "20,000 short texts took {:?}", started.elapsed());
}

/// A long command output, with lines full of secrets' names and unclosed quotes, is hidden in time.
#[test]
fn hidden_is_fast_on_long_text() {
    let started = std::time::Instant::now();
    let long = "TOKEN=' -u x password: \" {[ -pX \n".repeat(5_000);
    let _ = hidden(&long);
    let lines = "npm ERR! some line without a secret in it\n".repeat(50_000);
    assert_eq!(hidden(&lines), lines);
    assert!(started.elapsed().as_secs() < 10, "took {:?}", started.elapsed());
}

/// GA-281 QA, round 2: an address right after a character outside ASCII. Hersir runs git in the user's own language, and
/// git in Bulgarian (LANGUAGE=bg) puts `„` right before it, as in this line of git 2.x's, seen here:
/// `ФАТАЛНА ГРЕШКА: няма достъп до „https://…/“: …`. So do curly quotes and a zero-width space pasted with a link.
/// `hide_url_logins` cut the text inside that character and panicked (byte index … is not a char boundary).
#[test]
fn an_address_right_after_a_character_outside_ascii_is_hidden_without_a_panic() {
    let bg = "ФАТАЛНА ГРЕШКА: няма достъп до „https://me:hunter2@github.com/o/r.git/“: The requested URL returned error: 403";
    let out = std::panic::catch_unwind(|| hidden(bg)).expect("hidden panicked on git's Bulgarian message");
    assert_eq!(out, "ФАТАЛНА ГРЕШКА: няма достъп до „https://me:***@github.com/o/r.git/“: The requested URL returned error: 403");
    for text in ["“https://github.com/o/r”", "\u{200B}https://github.com/o/r", "→https://example.com/x", "€ssh://git@host/x"] {
        let out = std::panic::catch_unwind(|| hidden(text)).unwrap_or_else(|_| panic!("hidden panicked on {text:?}"));
        assert_eq!(out, text);
    }
    let out = std::panic::catch_unwind(|| hidden("«https://x-access-token:ghs_abc@github.com/o/r»"))
        .expect("hidden panicked on «https://…»");
    assert_eq!(out, "«https://x-access-token:***@github.com/o/r»");
}
