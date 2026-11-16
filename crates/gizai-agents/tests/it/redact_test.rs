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

/// Characters outside ASCII of every length (2, 3 and 4 bytes), marks without a width of their own (a zero-width space, a
/// right-to-left mark, a byte order mark, a combining accent) and letters in other scripts.
const OUTSIDE_ASCII: [&str; 18] = ["é", "„", "“", "”", "«", "»", "→", "€", "🙂", "\u{200B}", "\u{200F}", "\u{FEFF}",
                                   "\u{0301}", "Ａ", "ü", "Я", "中", "Ω"];

/// GA-281 QA, round 3: round 2's fix for the address after `„`, for every kind of character outside ASCII, right before
/// an address, inside its scheme and right after it: no panic, the password left out, the character kept.
#[test]
fn every_kind_of_character_outside_ascii_around_an_address_keeps_the_password_out() {
    for c in OUTSIDE_ASCII {
        let cases = [
            (format!("{c}https://me:hunter2@github.com/o/r{c}"), format!("{c}https://me:***@github.com/o/r{c}")),
            (format!("{c}{c}http://tok@host {c}"), format!("{c}{c}http://***@host {c}")),
            (format!("x{c}git+ssh://me:hunter2@host/{c}"), format!("x{c}git+ssh://me:***@host/{c}")),
            (format!("h{c}ttps://me:hunter2@host"), format!("h{c}ttps://me:***@host")),
            (format!("{c}ssh://git@host/x{c}"), format!("{c}ssh://git@host/x{c}")),
            (format!("'{c}https://me:hunter2@host{c}'"), format!("'{c}https://me:***@host{c}'")),
        ];
        for (text, want) in cases {
            let out = std::panic::catch_unwind(|| hidden(&text)).unwrap_or_else(|_| panic!("hidden panicked on {text:?}"));
            assert_eq!(out, want, "{text:?}");
        }
    }
}

/// GA-281 QA, round 3: the other rules (a secret's name and `=` or `:`, a flag, `Bearer`, mysql's `-p`, curl's `-u`)
/// with characters outside ASCII right before the name and around the value: no panic, the value left out whole and
/// what follows it kept.
#[test]
fn characters_outside_ascii_around_a_secrets_name_and_value_keep_the_value_out() {
    for c in OUTSIDE_ASCII {
        let cases = [
            (format!("{c}TOKEN={c}abc{c}xyz next"), " next"),
            (format!("{c} DB_PASSWORD=p@ss{c}abc{c}xyz npm ci"), " npm ci"),
            (format!("{c}Authorization: Bearer {c}abc{c}xyz\nnext"), "\nnext"),
            (format!("{c} --password {c}abc{c}xyz run"), " run"),
            (format!("{c} --api-key '{c}abc xyz{c}' run"), " run"),
            (format!("mysql -u root -p{c}abc{c}xyz db"), " db"),
            (format!("{c} curl -u me:{c}abc{c}xyz https://x.example.com"), " https://x.example.com"),
            (format!("{c}\"api_key\": \"{c}abc xyz{c}\", \"name\": \"x\""), ", \"name\": \"x\""),
            (format!("{c} Bearer {c}abc{c}xyz ok"), " ok"),
            (format!("client_secret: {c}abc {c}xyz\nname: {c}"), "\nname: "),
        ];
        for (text, kept) in cases {
            let out = std::panic::catch_unwind(|| hidden(&text)).unwrap_or_else(|_| panic!("hidden panicked on {text:?}"));
            assert!(!out.contains("abc") && !out.contains("xyz"), "{text:?} -> {out:?} still holds the secret");
            assert!(out.contains(kept) && out.contains(LEFT_OUT), "{text:?} -> {out:?} should keep {kept:?}");
        }
    }
}

/// GA-281 QA, round 3: a wider sweep than `hidden_never_panics_or_hangs_on_odd_text`, with every kind of character
/// outside ASCII next to the pieces the rules look at, and two planted secrets with a space on both sides: a password in
/// an address and a `PASSWORD=` value. Whatever stands around them, `hidden` must not panic and must leave both out (a
/// space on both sides means no rule can take them as part of something else and keep them). The one exception is curl's
/// `-u`/`--user` rule, which keeps everything before a login's `:` as its user name, also in a quoted value with spaces
/// (`--user=" PASSWORD=… …:"`): texts with `-u` are only checked for panics. GA281_SWEEP sets the number of texts
/// (default 40,000); QA ran it with 300,000.
#[test]
fn a_wide_sweep_with_characters_outside_ascii_never_panics_and_never_keeps_a_planted_secret() {
    let mut pieces = vec!["TOKEN", "password", "APP_KEY", "-p", "-u", "--user", "--token", "Bearer", "Basic", "Authorization",
                          "mysql", "=", "==", "=>", ":=", ":", ": ", " ", "\t", "\n", "\r\n", "'", "\"", "`", "\\", "{", "}",
                          "[", "]", "@", "://", "https://", "http://", "me:pw@", "ghp_", "eyJ", "a", "1", ".", "/", "-",
                          "x-access-token:", "sk-", "$", "#", "%", "?", "(", ")", ",", ";", "<", ">",
                          " https://me:Zq9pw@h/ ", " PASSWORD=Kx7pw "];
    pieces.extend(OUTSIDE_ASCII);
    let count: usize = std::env::var("GA281_SWEEP").ok().and_then(|n| n.parse().ok()).unwrap_or(40_000);
    let mut seed: u64 = 0x0281_2026_1116;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed >> 33
    };
    let started = std::time::Instant::now();
    for _ in 0..count {
        let len = next() % 32;
        let mut text = String::new();
        for _ in 0..len {
            text.push_str(pieces[(next() % pieces.len() as u64) as usize]);
        }
        let out = std::panic::catch_unwind(|| hidden(&text)).unwrap_or_else(|_| panic!("hidden panicked on {text:?}"));
        if text.contains("-u") {
            continue;
        }
        for planted in ["Zq9pw", "Kx7pw"] {
            assert!(!out.contains(planted), "{text:?} -> {out:?} still holds {planted:?}");
        }
    }
    assert!(started.elapsed().as_secs() < 60, "{count} texts took {:?}", started.elapsed());
}

/// GA-281 QA, round 3: one long line full of secrets (a minified JSON answer, a long command on one line) is hidden in
/// time too, not only many short lines (`hidden_is_fast_on_long_text`).
#[test]
fn one_long_line_full_of_secrets_is_hidden_in_time() {
    let started = std::time::Instant::now();
    let json = format!("{{{}\"end\": 1}}", "\"token\": \"abc def\", \"name\": \"x\", ".repeat(2_000));
    let out = hidden(&json);
    assert!(!out.contains("abc") && out.contains("\"name\": \"x\""), "{}", &out[..200]);
    let env = "TOKEN=abc DB_PASSWORD=p@ss ".repeat(2_000);
    let out = hidden(&env);
    assert!(!out.contains("abc") && !out.contains("p@ss"), "{}", &out[..200]);
    assert!(started.elapsed().as_secs() < 10, "took {:?}", started.elapsed());
}

/// GA-281 QA, round 3: a token with a known start (`TOKEN_STARTS`, "secret wherever they stand") used as a login's user
/// name rather than its password. GitHub takes a token as the user name (`https://<token>:x-oauth-basic@github.com`,
/// `curl -u <token>:`), and npm and pip take dependencies as `git+https://<token>@github.com/…`. A failed setup command's
/// first line, `<command> failed (exit code …)`, reaches Hersir's log (`runs::pause_pull`), so each of these setup
/// commands would put the whole token in it: the URL rule keeps a user name before `:` and every login of a scheme other
/// than http or https, curl's `-u` rule keeps the user name before `:`, and the token rule only looks at a word's start,
/// which is `//` here. Fails on purpose until a known token is left out wherever it stands.
#[test]
fn a_known_token_is_left_out_also_as_a_logins_user_name() {
    let t = "ghp_1234567890abcdefghijABCDEFGHIJ123456";
    let secret = "1234567890abcdefghij";
    for command in [
        format!("git clone https://{t}:x-oauth-basic@github.com/acme/lib.git vendor/lib failed (exit code 128)"),
        format!("curl -fsSL -u {t}: https://api.github.com/repos/acme/lib/tarball -o lib.tgz failed (exit code 22)"),
        format!("pip install git+https://{t}@github.com/acme/lib.git failed (exit code 1)"),
        format!("npm install git+https://{t}@github.com/acme/lib.git failed (exit code 1)"),
    ] {
        let out = hidden(first_line(&command));
        assert!(!out.contains(secret), "{command:?} -> {out:?} still holds the token");
        assert!(out.contains(" failed (exit code "), "{command:?} -> {out:?}");
    }
}

/// GA-281 QA, round 4: round 3's fix (a token as a login's user name) in the other forms a setup command can take it:
/// GitHub's older 40-character tokens, which have no known start; curl's login quoted, after `--user=` and with
/// x-oauth-basic; ssh and git+ssh; git's `url."…".insteadOf`; an empty password; package.json's address; and the
/// password logins of GitLab, Bitbucket and Azure DevOps.
#[test]
fn a_token_as_a_logins_user_name_is_left_out_in_every_form_a_setup_command_takes_it() {
    let t = "ghp_1234567890abcdefghijABCDEFGHIJ123456";
    let hex = "0123456789abcdef0123456789abcdef01234567";
    let cases = [
        (format!("git clone https://{hex}:x-oauth-basic@github.com/acme/lib.git"), "git clone https://***@github.com/acme/lib.git"),
        (format!("git clone https://{hex}@github.com/acme/lib.git"), "git clone https://***@github.com/acme/lib.git"),
        (format!("pip install git+https://{hex}@github.com/acme/lib.git"), "pip install git+https://***@github.com/acme/lib.git"),
        (format!("curl -u {hex}: https://api.github.com"), "curl -u *** https://api.github.com"),
        (format!("curl -u {hex}:x-oauth-basic https://api.github.com"), "curl -u *** https://api.github.com"),
        (format!("curl -u \"{t}:\" https://api.github.com"), "curl -u \"***\" https://api.github.com"),
        (format!("curl -u '{t}:' https://api.github.com"), "curl -u '***' https://api.github.com"),
        (format!("curl --user={t}: https://api.github.com"), "curl --user=*** https://api.github.com"),
        (format!("curl --user {t}:x-oauth-basic https://api.github.com"), "curl --user *** https://api.github.com"),
        (format!("curl -u {t} https://api.github.com"), "curl -u ghp_*** https://api.github.com"),
        (format!("ssh://{t}@github.com/acme/lib.git"), "ssh://***@github.com/acme/lib.git"),
        (format!("git+ssh://{t}@github.com/acme/lib.git"), "git+ssh://***@github.com/acme/lib.git"),
        (format!("git config --global url.\"https://{hex}:x-oauth-basic@github.com/\".insteadOf \"https://github.com/\""),
         "git config --global url.\"https://***@github.com/\".insteadOf \"https://github.com/\""),
        (format!("go get https://{t}:@github.com/acme/lib"), "go get https://***@github.com/acme/lib"),
        (format!("lib @ git+https://{t}@github.com/acme/lib.git@v1.0#egg=lib"), "lib @ git+https://***@github.com/acme/lib.git@v1.0#egg=lib"),
        (format!("\"lib\": \"git+https://{t}:x-oauth-basic@github.com/acme/lib.git#v1\""), "\"lib\": \"git+https://***@github.com/acme/lib.git#v1\""),
        ("https://oauth2:glpat-abcdefghij0123456789@gitlab.com/acme/lib.git".into(), "https://oauth2:***@gitlab.com/acme/lib.git"),
        (format!("https://x-token-auth:{hex}@bitbucket.org/acme/lib.git"), "https://x-token-auth:***@bitbucket.org/acme/lib.git"),
        (format!("https://{hex}@dev.azure.com/acme/lib/_git/lib"), "https://***@dev.azure.com/acme/lib/_git/lib"),
        (format!("echo {t}|gh auth login --with-token"), "echo ghp_***|gh auth login --with-token"),
        (format!("令牌{t}，请检查"), "令牌ghp_***，请检查"),
        (format!("username={t}"), "username=ghp_***"),
        (format!("({t})"), "(ghp_***)"),
    ];
    for (text, want) in cases {
        let out = std::panic::catch_unwind(|| hidden(&text)).unwrap_or_else(|_| panic!("hidden panicked on {text:?}"));
        assert_eq!(out, want, "{text:?}");
    }
}

/// GA-281 QA, round 4: what round 3's change must leave as it was: logins that are names (ssh's git@, ftp's anonymous@),
/// a password after a user name that is a name, `-u` that isn't curl's login, and words in a path that only look like a
/// token's start.
#[test]
fn names_paths_and_other_logins_stay_after_round_3s_change() {
    for text in ["ssh://git@github.com/acme/lib.git", "git@github.com:Yotech-AI/hersir.git", "git+ssh://git@github.com/acme/lib.git",
                 "ftp://anonymous@ftp.example.com/pub", "php artisan key:generate", "sudo -u www-data php artisan migrate",
                 "git push -u origin main", "git clone https://github.com/sk-hynix/some-long-repository-name.git",
                 "/home/me/task-runner-for-every-project-we-have/bin", "http://localhost:8000/login?next=/",
                 "fatal: unable to access 'https://github.com/acme/lib.git/': Could not resolve host"] {
        assert_eq!(hidden(text), text);
    }
    assert_eq!(hidden("curl -u me:hunter2 https://x.example.com"), "curl -u me:*** https://x.example.com");
    assert_eq!(hidden("https://x-access-token:ghs_16C7e42F292c6912E7710c838347Ae178B4a@github.com/o/r"),
               "https://x-access-token:***@github.com/o/r");
    assert_eq!(hidden("mysql://root:S3cretPw@127.0.0.1/app"), "mysql://root:***@127.0.0.1/app");
    assert_eq!(hidden("redis://:S3cretPw@127.0.0.1:6379"), "redis://:***@127.0.0.1:6379");
}

/// GA-281 QA, round 4: a secret given as a plain argument in a setup command, after a word that doesn't name it, with no
/// `=` and no known token start: Laravel Nova's and Flux's documented `composer config http-basic.<host> <user> <key>`,
/// composer's GitHub token, npm's token after a space (npm's own tokens start with `npm_`, which isn't a known start),
/// `docker login -p` and `sshpass -p`, and a token stuck to curl's `-u`. A failed setup command's first line, which
/// holds the whole command, reaches Hersir's log (`runs::pause_pull`), so each puts its secret there. Ignored: a known
/// gap left for Jefsev's decision on GA-281 (another rule per tool, or no setup command text in the log at all). Run it
/// with `cargo test -p gizai-agents --test it -- --ignored a_secret_given_as_a_plain_argument`.
#[test]
#[ignore = "GA-281 QA round 4: known gap, waiting for a decision"]
fn a_secret_given_as_a_plain_argument_in_a_setup_command_is_left_out() {
    let cases = [
        ("composer config http-basic.nova.laravel.com me@example.com nv-Lic3nseK3y && composer install failed (exit code 1)", "nv-Lic3nseK3y"),
        ("composer config --global github-oauth.github.com 0123456789abcdef0123456789abcdef01234567 failed (exit code 1)", "0123456789abcdef"),
        ("npm config set //registry.npmjs.org/:_authToken npm_a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8 && npm ci failed (exit code 1)", "a1B2c3D4"),
        ("docker login -u me -p S3cretPw ghcr.io failed (exit code 1)", "S3cretPw"),
        ("sshpass -p S3cretPw ssh deploy@host failed (exit code 255)", "S3cretPw"),
        ("curl -ughp_1234567890abcdefghijABCDEFGHIJ123456: https://api.github.com failed (exit code 22)", "1234567890abcdefghij"),
    ];
    for (text, secret) in cases {
        let out = hidden(first_line(text));
        assert!(!out.contains(secret), "{text:?} -> {out:?} still holds {secret:?}");
    }
}
