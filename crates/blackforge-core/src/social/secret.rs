//! Decides which settings never leave this computer. A few mods keep a
//! Discord webhook address, a bot token or a remote console password in their
//! config, and everything that is shared is readable by every friend.

/// Compared against the key with everything but letters and digits removed,
/// so `API Key`, `api_key` and `ApiKey` are all `apikey`.
const SECRET_WORDS: [&str; 5] = ["password", "token", "secret", "webhook", "apikey"];

const ADDRESS_STARTS: [&str; 4] = ["http://", "https://", "ws://", "wss://"];

/// The word list is a guess on purpose. A harmless setting that is kept back
/// costs nothing, a leaked webhook lets anyone post into that channel.
pub fn looks_secret(key: &str, value: &str) -> bool {
    let key: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|letter| letter.to_ascii_lowercase())
        .collect();
    if SECRET_WORDS.iter().any(|word| key.contains(word)) {
        return true;
    }

    let value = value.trim().to_ascii_lowercase();
    ADDRESS_STARTS.iter().any(|start| value.starts_with(start))
}

#[cfg(test)]
mod tests {
    use super::looks_secret;

    #[test]
    fn secret_words_in_any_spelling() {
        assert!(looks_secret("Password", "hunter2"));
        assert!(looks_secret("RCON password", "x"));
        assert!(looks_secret("Bot Token", "x"));
        assert!(looks_secret("client_secret", "x"));
        assert!(looks_secret("Webhook URL", ""));
        assert!(looks_secret("API Key", "x"));
        assert!(looks_secret("api_key", "x"));
    }

    #[test]
    fn a_web_address_is_secret_under_any_key() {
        assert!(looks_secret(
            "Url",
            "https://discord.com/api/webhooks/1/abc"
        ));
        assert!(looks_secret("Endpoint", "  HTTP://example.com "));
        assert!(looks_secret("Socket", "wss://example.com"));
    }

    #[test]
    fn plain_settings_are_shared() {
        assert!(!looks_secret("Enabled", "true"));
        assert!(!looks_secret("Count", "9"));
        assert!(!looks_secret("Hotkey", "LeftControl"));
        assert!(!looks_secret("Message", "see https in the docs"));
    }
}
