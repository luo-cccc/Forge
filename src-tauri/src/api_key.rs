const KEYRING_SERVICE: &str = "agent-writer";

fn load_api_key_from_keychain() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, "openai").ok()?;
    entry.get_password().ok()
}

pub(crate) fn resolve_api_key() -> Option<String> {
    load_api_key_from_keychain()
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        .filter(|k| !k.is_empty())
}

pub(crate) fn require_api_key() -> Result<String, String> {
    resolve_api_key().ok_or_else(|| "API key not set. Go to Settings.".to_string())
}
