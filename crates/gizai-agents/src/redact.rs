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

/// A word holding one of these (in any case) names a secret: the word after it is its value when `=` or `:` comes
/// between (`TOKEN=…`, `"api_key": "…"`, `Authorization: …`), or a space after a flag (`--password …`).
const SECRET_NAMES: [&str; 7] = ["token", "secret", "password", "passwd", "apikey", "api_key", "authorization"];

/// What stands where something was left out.
pub const LEFT_OUT: &str = "***";

/// `text` with what looks like a password, key or token left out (`***`):
/// - the password in a URL (`https://me:***@host`), and a URL's whole login over HTTP and HTTPS, where it is a token
///   (`https://***@github.com`); `ssh://git@host` and `git@host:owner/repo` stay as they are;
/// - tokens by their known start (`TOKEN_STARTS`: `ghp_***`, `github_pat_***`, `sk-***` …) and JSON web tokens (`eyJ***`);
/// - the value after a name with a secret in it (`SECRET_NAMES`) and `=` or `:` (`PASSWORD=***`, `"token": "***"`), or
///   after such a flag and a space (`--password ***`), and the word after `Bearer` or `Basic`.
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

/// `text` with secret words left out: tokens by their start, and the value after a secret's name, a flag or a scheme.
fn hide_words(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || "_-.~+/".contains(c);
    // the text in runs of word characters and of the rest, in order
    let mut runs: Vec<(bool, &str)> = vec![];
    let mut start = 0;
    let mut kind = None;
    for (j, c) in text.char_indices() {
        let w = is_word(c);
        if kind != Some(w) {
            if let Some(k) = kind {
                runs.push((k, &text[start..j]));
            }
            start = j;
            kind = Some(w);
        }
    }
    if let Some(k) = kind {
        runs.push((k, &text[start..]));
    }
    let mut out = String::with_capacity(text.len());
    let mut hide_next = false;
    for (n, (word, part)) in runs.iter().enumerate() {
        if !word {
            out.push_str(part);
            continue;
        }
        if hide_next {
            out.push_str(LEFT_OUT);
        } else {
            out.push_str(&token_hidden(part));
        }
        let between = match runs.get(n + 1) {
            Some((false, s)) => *s,
            _ => "",
        };
        let lower = part.to_ascii_lowercase();
        let names_secret = SECRET_NAMES.iter().any(|name| lower.contains(name));
        let assigns = between.contains(['=', ':']) && between.chars().all(|c| " \t=:'\"".contains(c));
        let spaced = !between.is_empty() && between.chars().all(|c| c == ' ' || c == '\t');
        hide_next = (names_secret && assigns)
            || (names_secret && part.starts_with('-') && spaced)
            || ((lower == "bearer" || lower == "basic") && spaced);
    }
    out
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
