//! Text from outside Hersir as Hersir's own log may show it (GA-281): users send us that log (`src-tauri/src/logs.rs`),
//! so a password, key or token in git's or gh's words, in a command's error or in a project's setup command is left out
//! first (`hidden`). Plain text work, the same on every system.

/// Starts of tokens that are secret wherever they stand, each with how many characters must follow before a word counts
/// as one: GitHub's (personal, fine-grained, OAuth, user, app and refresh tokens), GitLab's, Anthropic's and OpenAI's
/// (`sk-`), Slack's, Atlassian's API tokens (Bitbucket's too) and AWS's access keys.
const TOKEN_STARTS: [(&str, usize); 15] = [
    ("ghp_", 20), ("gho_", 20), ("ghu_", 20), ("ghs_", 20), ("ghr_", 20), ("github_pat_", 20), ("glpat-", 16),
    ("sk-", 20), ("xoxb-", 10), ("xoxp-", 10), ("xoxa-", 10), ("xoxr-", 10), ("xoxs-", 10), ("ATATT", 20), ("AKIA", 16),
];

/// A word holding one of these (in any case) names a secret: the value after it is left out when `=` or `:` comes
/// between (`TOKEN=…`, `"api_key": "…"`, `Authorization: …`), or a space after a flag (`--password …`).
const SECRET_NAMES: [&str; 12] = ["token", "secret", "password", "passwd", "passphrase", "credential", "authorization",
                                  "apikey", "api_key", "api-key", "cookie", "webhook"];

/// A word ending in one of these (in any case) names a secret too: `OPENAI_KEY`, `APP_KEY`, `apiKey`, `MYSQL_PWD`,
/// `DB_PASS`, `COMPOSER_AUTH`.
const SECRET_ENDS: [&str; 4] = ["key", "pwd", "pass", "auth"];

/// The quotes a value can stand in.
const QUOTES: [char; 3] = ['\'', '"', '`'];

/// What stands where something was left out.
pub const LEFT_OUT: &str = "***";

/// `text` with what looks like a password, key or token left out (`***`):
/// - the password in a URL (`https://me:***@host`), and a URL's whole login over HTTP and HTTPS, where it is a token
///   (`https://***@github.com`); `ssh://git@host` and `git@host:owner/repo` stay as they are;
/// - tokens by their known start (`TOKEN_STARTS`: `ghp_***`, `github_pat_***`, `sk-***` …) and JSON web tokens (`eyJ***`);
/// - the whole value after a name that names a secret (`SECRET_NAMES`, `SECRET_ENDS`) and `=` or `:` (`PASSWORD=***`,
///   `APP_KEY=***`, `"token": "***"`), or after such a flag and a space (`--password ***`), whatever characters it holds:
///   a quoted one up to its closing quote (`'***'`), one in braces or brackets up to the one that closes them, one after
///   `:` or after a sign with a space after it up to the line's end, each word `***` (`client_secret: *** ***`,
///   `Authorization: *** ***`), and any other up to a space, as a shell reads `TOKEN=*** npm ci` (`value_end`). A value
///   in quotes ends at the quote around it (`'Authorization: *** ***'`);
/// - the value after `Bearer` or `Basic`;
/// - mysql's and mariadb's password stuck to `-p` (`-p***`), and the password in a login after `-u` or `--user`, as curl
///   takes it (`-u me:***`).
///
/// Everything else stays as it was, line breaks included.
pub fn hidden(text: &str) -> String {
    hide_words(&hide_url_logins(text))
}

/// The first line of `text` that holds something, trimmed; "" for none. For a reason whose further lines are a
/// command's output (a failed setup command), which stays on its card.
pub fn first_line(text: &str) -> &str {
    text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("")
}

/// `text` with the logins in its URLs left out: after `scheme://`, up to the host's `@`.
fn hide_url_logins(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        let (before, after) = rest.split_at(i + 3);
        out.push_str(before);
        let scheme_start = before[..i].rfind(|c: char| !(c.is_ascii_alphanumeric() || "+.-".contains(c))).map_or(0, |j| j + 1);
        let scheme = before[scheme_start..i].to_ascii_lowercase();
        // the address's authority: its login, host and port
        let end = after.find(|c: char| c.is_whitespace() || "/?#'\"<>()[],;`".contains(c)).unwrap_or(after.len());
        let authority = &after[..end];
        match authority.rfind('@') {
            Some(at) => {
                let login = &authority[..at];
                match login.find(':') {
                    Some(colon) => {
                        out.push_str(&login[..=colon]);
                        out.push_str(LEFT_OUT);
                    }
                    None if scheme == "http" || scheme == "https" => out.push_str(LEFT_OUT),
                    None => out.push_str(login),
                }
                out.push_str(&authority[at..]);
            }
            None => out.push_str(authority),
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// `text` with secret words left out: tokens by their start, and the values `secret_value` finds, each whole.
fn hide_words(text: &str) -> String {
    // mysql's and mariadb's password sticks to its flag (`-pS3cret`), which means other things elsewhere
    let lower = text.to_ascii_lowercase();
    let mysql = lower.contains("mysql") || lower.contains("mariadb");
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(c) = text[i..].chars().next() {
        if !is_word(c) {
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        let end = text[i..].find(|c: char| !is_word(c)).map_or(text.len(), |n| i + n);
        let word = &text[i..end];
        let Some(Value { mut at, mut quote, to_line_end }) = secret_value(text, i, end, mysql) else {
            out.push_str(&token_hidden(word));
            i = end;
            continue;
        };
        if at < end {
            // `-pS3cret`
            out.push_str(&text[i..at]);
        } else {
            out.push_str(&token_hidden(word));
            out.push_str(&text[end..at]);
        }
        // in `echo "APP_KEY=" >> .env` that quote closes the one the name stands in: the value is what follows it at once
        if let Some(q) = quote && text[..i].ends_with(q) && !text[end..at - 1].contains(q) {
            quote = None;
        }
        let outer = outer_quote(text, i, at);
        if to_line_end && quote.is_none() && !text[at..].starts_with(['{', '[']) {
            // `client_secret: p@ss w0rd`, `Authorization: Bearer …`: up to the line's end or the quote around it, each
            // word left out and the spaces between kept
            let line_end = text[at..].find(['\n', '\r']).map_or(text.len(), |n| at + n);
            let stop = outer.and_then(|q| closing(text, at, q, true)).map_or(line_end, |k| k.min(line_end));
            let mut in_word = false;
            for c in text[at..stop].chars() {
                if c.is_whitespace() {
                    out.push(c);
                } else if !in_word {
                    out.push_str(LEFT_OUT);
                }
                in_word = !c.is_whitespace();
            }
            i = stop;
            continue;
        }
        // after `Authorization=` its scheme and then its credentials go, and after a value `Bearer` or `Basic` the next one
        let mut and_next = word.to_ascii_lowercase().contains("authorization");
        loop {
            let stop = value_end(text, at, quote, outer);
            let value = &text[at..stop];
            if !value.is_empty() {
                out.push_str(LEFT_OUT);
            }
            i = stop;
            if quote.is_some() || !(and_next || is_scheme(value)) {
                break;
            }
            and_next = false;
            let next = text.len() - text[i..].trim_start_matches([' ', '\t']).len();
            match text[next..].chars().next() {
                Some(c) if next > i && c != '\n' && c != '\r' && Some(c) != outer => {
                    (at, quote) = opening(text, next);
                    out.push_str(&text[i..at]);
                }
                _ => break,
            }
        }
    }
    out
}

/// Whether `c` belongs in a word, as names, flags, paths and tokens have them.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || "_-.~+/".contains(c)
}

/// Whether a word in lower case names a secret (`SECRET_NAMES`, `SECRET_ENDS`).
fn names_secret(lower: &str) -> bool {
    SECRET_NAMES.iter().any(|name| lower.contains(name)) || SECRET_ENDS.iter().any(|end| lower.ends_with(end))
}

/// Whether `word` is an Authorization scheme that a secret follows.
fn is_scheme(word: &str) -> bool {
    word.eq_ignore_ascii_case("bearer") || word.eq_ignore_ascii_case("basic")
}

/// A secret value found after a word: where it starts, the quote it starts after (it ends at its closing one), and
/// whether, unquoted, it runs to its line's end rather than to a space.
struct Value {
    at: usize,
    quote: Option<char>,
    to_line_end: bool,
}

/// The value at `at`, after its opening quote if it starts with one.
fn value_at(text: &str, at: usize, to_line_end: bool) -> Value {
    let (at, quote) = opening(text, at);
    Value { at, quote, to_line_end }
}

/// The secret value after the word `text[start..end]`, if one follows it:
/// - after a word that names a secret and `=` or `:` (`assignment`);
/// - after a flag that names a secret and spaces: `--password …`, `--api-key '…'`;
/// - after `Bearer` or `Basic` and spaces;
/// - in a text about mysql or mariadb, the password stuck to `-p`: `-pS3cret`, `-p'…'`;
/// - after `-u` or `--user`, the password after a login's `:`: `-u me:…`.
fn secret_value(text: &str, start: usize, end: usize, mysql: bool) -> Option<Value> {
    let word = &text[start..end];
    let rest = &text[end..];
    if names_secret(&word.to_ascii_lowercase()) {
        if let Some((n, to_line_end)) = assignment(rest) {
            return Some(value_at(text, end + n, to_line_end));
        }
        if word.starts_with('-') && let Some(n) = spaced(rest) {
            return Some(value_at(text, end + n, false));
        }
    }
    if is_scheme(word) && let Some(n) = spaced(rest) {
        return Some(value_at(text, end + n, false));
    }
    if mysql && word.starts_with("-p") && !word.starts_with("--") && text[start + 2..].starts_with(|c: char| !c.is_whitespace()) {
        return Some(value_at(text, start + 2, false));
    }
    if word == "-u" || word == "--user" {
        let n = spaced(rest).or_else(|| (word == "--user" && rest.starts_with('=')).then_some(1))?;
        let Value { at, quote, .. } = value_at(text, end + n, false);
        let login_end = value_end(text, at, quote, outer_quote(text, start, at));
        let colon = text[at..login_end].find(':')?;
        return Some(Value { at: at + colon + 1, quote, to_line_end: false });
    }
    None
}

/// Where a value starts after a word that names a secret, when `=` or `:` (also `=>` and `:=`) follows it: after the
/// word's closing quote and spaces, if any, the sign, and spaces (`=`, `: `, `": "`, `' => '`). A `:` needs a space or a
/// quote after it, as in YAML, JSON and headers, so `key:generate` and `token:***@host` hold no value; `==` is none.
/// Also whether the value runs to the line's end: after `:` (YAML, headers) and after a sign with a space after it
/// (`password = …` in INI, TOML and Makefiles); not after `=` without one, where a shell ends it at a space
/// (`TOKEN=… npm ci`).
fn assignment(rest: &str) -> Option<(usize, bool)> {
    let n = rest.len() - rest.trim_start_matches([' ', '\t', '\'', '"', '`']).len();
    let after = &rest[n..];
    let sign = if after.starts_with("=>") || after.starts_with(":=") {
        2
    } else if (after.starts_with('=') && !after.starts_with("=="))
        || (after.starts_with(':') && after[1..].starts_with([' ', '\t', '\'', '"', '`'])) {
        1
    } else {
        return None;
    };
    let value = rest[n + sign..].trim_start_matches([' ', '\t']);
    let spaces = rest.len() - n - sign - value.len();
    Some((rest.len() - value.len(), after.starts_with(':') || spaces > 0))
}

/// Where a value starts after the spaces `rest` starts with, when something follows them on the same line.
fn spaced(rest: &str) -> Option<usize> {
    let n = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    (n > 0 && rest[n..].starts_with(|c: char| c != '\n' && c != '\r')).then_some(n)
}

/// `at`, or the place after the quote at `at` that a value starts with, and that quote.
fn opening(text: &str, at: usize) -> (usize, Option<char>) {
    match text[at..].chars().next() {
        Some(q) if QUOTES.contains(&q) => (at + 1, Some(q)),
        _ => (at, None),
    }
}

/// The quote around the value at `at` after the word at `start`, which ends the value: the one right before that word,
/// when nothing in between closes it (`'Authorization: Bearer …'`), else a `"` or `` ` `` the line opened before the
/// value and didn't close (`sh -c "… TOKEN=…"`).
fn outer_quote(text: &str, start: usize, at: usize) -> Option<char> {
    if let Some(q) = text[..start].chars().next_back().filter(|c| QUOTES.contains(c)) && !text[start..at].contains(q) {
        return Some(q);
    }
    let line = &text[text[..at].rfind('\n').map_or(0, |n| n + 1)..at];
    ['"', '`'].into_iter().find(|&q| unclosed(line, q))
}

/// Whether `line` leaves a `q` open: an odd number of them, one after `\` not counted.
fn unclosed(line: &str, q: char) -> bool {
    let (mut open, mut escaped) = (false, false);
    for c in line.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == q {
            open = !open;
        }
    }
    open
}

/// Where the value at `at` ends, whatever characters it holds:
/// - one that started after the quote `quote`, at its closing quote (`closing`), also on a later line; without one, at
///   the text's end;
/// - one in braces or brackets (`{"token": …}`), after the one that closes them; without one, at the text's end;
/// - any other at a space or a line's end, or at `outer`, the quote it stands in. A quoted part in it and a character
///   after `\` are part of it (`PASSWORD=ab'c d'`, `ab\ c`), and so is a quote with no partner on its line, unless the
///   value ends right after it: that one closes a quote around the value (`sh -c "… TOKEN=abc"`).
fn value_end(text: &str, at: usize, quote: Option<char>, outer: Option<char>) -> usize {
    if let Some(q) = quote {
        return closing(text, at, q, false).unwrap_or(text.len());
    }
    if let Some(open @ ('{' | '[')) = text[at..].chars().next() {
        return bracketed_end(text, at, open).unwrap_or(text.len());
    }
    let mut n = at;
    while let Some(c) = text[n..].chars().next() {
        if c.is_whitespace() || Some(c) == outer {
            return n;
        }
        let mut next = n + c.len_utf8();
        if c == '\\' {
            next += text[next..].chars().next().map_or(0, char::len_utf8);
        } else if QUOTES.contains(&c) {
            match closing(text, next, c, true) {
                Some(k) => next = k + 1,
                None if text[next..].chars().next().is_none_or(char::is_whitespace) => return n,
                None => {}
            }
        }
        n = next;
    }
    text.len()
}

/// Where the quote `q` that opened before `from` closes: at the next `q`, one after `\` not counted in `"…"` and
/// `` `…` ``; with `same_line`, only on the same line.
fn closing(text: &str, from: usize, q: char, same_line: bool) -> Option<usize> {
    let mut escaped = false;
    for (n, c) in text[from..].char_indices() {
        if escaped {
            escaped = false;
        } else if c == q {
            return Some(from + n);
        } else if same_line && c == '\n' {
            return None;
        } else {
            escaped = c == '\\' && q != '\'';
        }
    }
    None
}

/// The end of the braces or brackets that `open` at `at` opens, with quoted parts in them skipped: after the one that
/// closes them.
fn bracketed_end(text: &str, at: usize, open: char) -> Option<usize> {
    let close = if open == '{' { '}' } else { ']' };
    let mut depth = 0usize;
    let mut n = at;
    while let Some(c) = text[n..].chars().next() {
        let mut next = n + c.len_utf8();
        if c == '"' {
            next = closing(text, next, c, false)? + 1;
        } else if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(next);
            }
        }
        n = next;
    }
    None
}

/// `word`, or its known start and `***` when it is a token (`TOKEN_STARTS`, a JSON web token).
fn token_hidden(word: &str) -> String {
    for (start, after) in TOKEN_STARTS {
        if word.len() >= start.len() + after && word.starts_with(start) {
            return format!("{start}{LEFT_OUT}");
        }
    }
    // a JSON web token: three parts with dots between, the first a JSON object's start in base64
    if word.starts_with("eyJ") && word.len() >= 30 && word.matches('.').count() == 2 {
        return format!("eyJ{LEFT_OUT}");
    }
    word.to_string()
}
