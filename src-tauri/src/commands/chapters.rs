use crate::storage::ChapterInfo;

#[tauri::command]
pub fn read_project_dir(app: tauri::AppHandle) -> Result<Vec<ChapterInfo>, String> {
    crate::storage::read_project_dir(&app)
}

#[tauri::command]
pub fn create_chapter(app: tauri::AppHandle, title: String) -> Result<ChapterInfo, String> {
    let chapter = crate::storage::create_chapter(&app, title.clone())?;
    crate::audit_project_file_write(
        &app,
        &title,
        &format!("Chapter created: {}", title),
        "created_chapter",
        &format!("Author created chapter '{}'.", title),
        &[format!("chapter:{}", title)],
    );
    Ok(chapter)
}

#[tauri::command]
pub fn load_chapter(app: tauri::AppHandle, title: String) -> Result<String, String> {
    crate::storage::load_chapter(&app, title)
}

#[tauri::command]
pub fn get_chapter_revision(app: tauri::AppHandle, title: String) -> Result<String, String> {
    crate::storage::chapter_revision(&app, &title)
}
