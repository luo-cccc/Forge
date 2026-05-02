const KEYRING_SERVICE: &str = "agent-writer";

#[tauri::command]
pub fn set_api_key(provider: String, key: String) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &provider)
        .map_err(|e| format!("Keyring error: {}", e))?;
    entry
        .set_password(&key)
        .map_err(|e| format!("Set error: {}", e))
}

#[tauri::command]
pub fn check_api_key(provider: String) -> Result<bool, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &provider)
        .map_err(|e| format!("Keyring error: {}", e))?;
    Ok(entry.get_password().is_ok())
}
