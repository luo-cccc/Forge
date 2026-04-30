use tauri;

#[tauri::command]
fn harness_echo(message: String) -> String {
    format!("Harness Received: {}", message)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![harness_echo])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
