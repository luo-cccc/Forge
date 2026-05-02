use crate::storage::OutlineNode;

#[tauri::command]
pub fn get_outline(app: tauri::AppHandle) -> Result<Vec<OutlineNode>, String> {
    crate::storage::load_outline(&app)
}

#[tauri::command]
pub fn save_outline_node(
    app: tauri::AppHandle,
    chapter_title: String,
    summary: String,
) -> Result<Vec<OutlineNode>, String> {
    let nodes = crate::storage::upsert_outline_node(&app, chapter_title.clone(), summary.clone())?;
    crate::audit_project_file_write(
        &app,
        &chapter_title,
        &format!("Outline saved: {}", chapter_title),
        "saved_outline_node",
        &format!(
            "Author saved outline for '{}' ({} chars).",
            chapter_title,
            summary.chars().count()
        ),
        &[format!("outline:{}", chapter_title)],
    );
    Ok(nodes)
}

#[tauri::command]
pub fn delete_outline_node(
    app: tauri::AppHandle,
    chapter_title: String,
) -> Result<Vec<OutlineNode>, String> {
    let nodes = crate::storage::remove_outline_node(&app, chapter_title.clone())?;
    crate::audit_project_file_write(
        &app,
        &chapter_title,
        &format!("Outline deleted: {}", chapter_title),
        "deleted_outline_node",
        &format!("Author deleted outline node '{}'.", chapter_title),
        &[format!("outline:{}", chapter_title)],
    );
    Ok(nodes)
}

#[tauri::command]
pub fn update_outline_status(
    app: tauri::AppHandle,
    chapter_title: String,
    status: String,
) -> Result<Vec<OutlineNode>, String> {
    let nodes = crate::storage::update_outline_status(&app, chapter_title.clone(), status.clone())?;
    crate::audit_project_file_write(
        &app,
        &chapter_title,
        &format!("Outline status: {}", chapter_title),
        "updated_outline_status",
        &format!(
            "Author set outline status for '{}' to '{}'.",
            chapter_title, status
        ),
        &[format!("outline:{}", chapter_title)],
    );
    Ok(nodes)
}

#[tauri::command]
pub fn reorder_outline_nodes(
    app: tauri::AppHandle,
    ordered_titles: Vec<String>,
) -> Result<Vec<OutlineNode>, String> {
    let nodes = crate::storage::reorder_outline_nodes(&app, ordered_titles.clone())?;
    crate::audit_project_file_write(
        &app,
        "outline",
        "Outline reordered",
        "reordered_outline",
        &format!("Author reordered {} outline nodes.", ordered_titles.len()),
        &["outline:order".to_string()],
    );
    Ok(nodes)
}
