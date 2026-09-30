//! A guard against settings that carry secrets. Tokens and keys belong in
//! the OS keyring; anything that looks like one is refused before it is
//! committed to the sync repository.

/// Well-known token prefixes, and how long the token after them is at least.
const PREFIXES: &[(&str, usize, &str)] = &[
    ("ghp_", 30, "a GitHub token"),
    ("gho_", 30, "a GitHub token"),
    ("ghu_", 30, "a GitHub token"),
    ("ghs_", 30, "a GitHub token"),
    ("ghr_", 30, "a GitHub token"),
    ("github_pat_", 30, "a GitHub token"),
    ("glpat-", 20, "a GitLab token"),
    ("xoxb-", 20, "a Slack token"),
    ("xoxp-", 20, "a Slack token"),
    ("AKIA", 16, "an AWS access key"),
    ("AIza", 30, "a Google API key"),
    ("ATATT", 30, "an Atlassian token"),
    ("npm_", 30, "an npm token"),
    ("sk-", 32, "an API key"),
];

/// Setting names that should never hold a value in a settings file.
const NAMES: &[&str] = &[
    "password",
    "passwd",
    "passphrase",
    "secret",
    "token",
    "apikey",
    "api_key",
    "privatekey",
    "private_key",
];

/// Why `text` looks like it contains a secret, if it does.
pub fn find_secret(text: &str) -> Option<String> {
    for (i, line) in text.lines().enumerate() {
        let at = |what: &str| Some(format!("line {} looks like {what}", i + 1));
        if line.contains("-----BEGIN") && line.contains("PRIVATE KEY") {
            return at("a private key");
        }
        for &(prefix, min, what) in PREFIXES {
            let mut rest = line;
            while let Some(pos) = rest.find(prefix) {
                let before = rest[..pos].chars().next_back();
                let after = &rest[pos + prefix.len()..];
                let len = after
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .count();
                if len >= min && !before.is_some_and(|c| c.is_ascii_alphanumeric()) {
                    return at(what);
                }
                rest = after;
            }
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().trim_matches('"').to_ascii_lowercase();
            let value = value.trim().trim_matches(['"', '\'']);
            if !value.is_empty() && NAMES.iter().any(|n| key.ends_with(n)) {
                return at("a secret setting");
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_tokens_keys_and_secret_settings() {
        let token = format!("ghp_{}", "a1".repeat(18));
        assert_eq!(
            find_secret(&format!("[ui]\nx = \"{token}\"\n")).as_deref(),
            Some("line 2 looks like a GitHub token")
        );
        assert!(find_secret("k = \"-----BEGIN OPENSSH PRIVATE KEY-----\"").is_some());
        assert!(find_secret("apiToken = \"abc\"").is_some());
        assert!(find_secret(&format!("x = \"AKIA{}\"", "ABCD".repeat(4))).is_some());
    }

    #[test]
    fn leaves_ordinary_settings_alone() {
        let text = r#"
[ui]
theme = "dark"
[keybindings]
"palette.open" = "Mod+Shift+P"
"sync.now" = ""
[[profiles]]
id = "default"
signingKey = "~/.ssh/id_ed25519.pub"
sshKey = "~/.ssh/id_ed25519"
userEmail = "task-ghp_short@example.com"
"#;
        assert_eq!(find_secret(text), None);
        // Empty secret-looking settings are fine.
        assert_eq!(find_secret("token = \"\""), None);
    }
}
