use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoreEntry {
    pub id: String,
    pub keyword: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChapterInfo {
    pub title: String,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineNode {
    pub chapter_title: String,
    pub summary: String,
    pub status: String,
}

pub fn app_data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    Ok(dir)
}

pub fn lorebook_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app_data_dir(app)?.join("lorebook.json"))
}

pub fn load_lorebook(app: &tauri::AppHandle) -> Result<Vec<LoreEntry>, String> {
    let path = lorebook_path(app)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&data).unwrap_or_else(|_| Ok(vec![]))
}

pub fn save_lorebook(app: &tauri::AppHandle, entries: &[LoreEntry]) -> Result<(), String> {
    let path = lorebook_path(app)?;
    let json = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

pub fn upsert_lore_entry(
    app: &tauri::AppHandle,
    keyword: String,
    content: String,
) -> Result<Vec<LoreEntry>, String> {
    let mut entries = load_lorebook(app)?;
    if let Some(entry) = entries.iter_mut().find(|e| e.keyword == keyword) {
        entry.content = content;
    } else {
        let id = (entries.len() + 1).to_string();
        entries.push(LoreEntry {
            id,
            keyword,
            content,
        });
    }
    save_lorebook(app, &entries)?;
    Ok(entries)
}

pub fn remove_lore_entry(app: &tauri::AppHandle, id: String) -> Result<Vec<LoreEntry>, String> {
    let mut entries = load_lorebook(app)?;
    entries.retain(|e| e.id != id);
    save_lorebook(app, &entries)?;
    Ok(entries)
}

pub fn project_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app_data_dir(app)?.join("project");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create project dir: {}", e))?;
    Ok(dir)
}

pub fn chapter_filename(title: &str) -> String {
    format!("{}.md", title.replace(' ', "-").to_lowercase())
}

pub fn read_project_dir(app: &tauri::AppHandle) -> Result<Vec<ChapterInfo>, String> {
    let dir = project_dir(app)?;
    let mut chapters = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let title = stem.replace('-', " ");
            chapters.push(ChapterInfo {
                filename: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                title,
            });
        }
    }
    chapters.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(chapters)
}

pub fn create_chapter(app: &tauri::AppHandle, title: String) -> Result<ChapterInfo, String> {
    let dir = project_dir(app)?;
    let filename = chapter_filename(&title);
    let path = dir.join(&filename);
    if !path.exists() {
        atomic_write(&path, "")?;
    }
    Ok(ChapterInfo { title, filename })
}

pub fn save_chapter_content(
    app: &tauri::AppHandle,
    title: &str,
    content: &str,
) -> Result<(), String> {
    let dir = project_dir(app)?;
    let path = dir.join(chapter_filename(title));
    atomic_write(&path, content)
}

pub fn load_chapter(app: &tauri::AppHandle, title: String) -> Result<String, String> {
    let dir = project_dir(app)?;
    let path = dir.join(chapter_filename(&title));
    if !path.exists() {
        return Err(format!("Chapter '{}' not found", title));
    }
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

pub fn outline_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app_data_dir(app)?.join("outline.json"))
}

pub fn load_outline(app: &tauri::AppHandle) -> Result<Vec<OutlineNode>, String> {
    let path = outline_path(app)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&data).unwrap_or_else(|_| Ok(vec![]))
}

pub fn save_outline(app: &tauri::AppHandle, nodes: &[OutlineNode]) -> Result<(), String> {
    let path = outline_path(app)?;
    let json = serde_json::to_string_pretty(nodes).map_err(|e| e.to_string())?;
    atomic_write(&path, &json)
}

pub fn upsert_outline_node(
    app: &tauri::AppHandle,
    chapter_title: String,
    summary: String,
) -> Result<Vec<OutlineNode>, String> {
    let mut nodes = load_outline(app)?;
    let status = if let Some(existing) = nodes.iter().find(|n| n.chapter_title == chapter_title) {
        existing.status.clone()
    } else {
        "empty".to_string()
    };
    if let Some(node) = nodes.iter_mut().find(|n| n.chapter_title == chapter_title) {
        node.summary = summary;
    } else {
        nodes.push(OutlineNode {
            chapter_title,
            summary,
            status,
        });
    }
    save_outline(app, &nodes)?;
    Ok(nodes)
}

pub fn remove_outline_node(
    app: &tauri::AppHandle,
    chapter_title: String,
) -> Result<Vec<OutlineNode>, String> {
    let mut nodes = load_outline(app)?;
    nodes.retain(|n| n.chapter_title != chapter_title);
    save_outline(app, &nodes)?;
    Ok(nodes)
}

pub fn update_outline_status(
    app: &tauri::AppHandle,
    chapter_title: String,
    status: String,
) -> Result<Vec<OutlineNode>, String> {
    let mut nodes = load_outline(app)?;
    if let Some(node) = nodes.iter_mut().find(|n| n.chapter_title == chapter_title) {
        node.status = status;
    }
    save_outline(app, &nodes)?;
    Ok(nodes)
}

pub fn reorder_outline_nodes(
    app: &tauri::AppHandle,
    ordered_titles: Vec<String>,
) -> Result<Vec<OutlineNode>, String> {
    let nodes = load_outline(app)?;
    let mut reordered = Vec::with_capacity(nodes.len());

    for title in &ordered_titles {
        if let Some(node) = nodes.iter().find(|n| &n.chapter_title == title) {
            reordered.push(node.clone());
        }
    }

    for node in nodes {
        if !ordered_titles
            .iter()
            .any(|title| title == &node.chapter_title)
        {
            reordered.push(node);
        }
    }

    save_outline(app, &reordered)?;
    Ok(reordered)
}

pub fn brain_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app_data_dir(app)?.join("project_brain.json"))
}

pub fn rename_chapter_file(
    app: &tauri::AppHandle,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let dir = project_dir(app)?;
    let old_path = dir.join(&old_name);
    let new_path = dir.join(&new_name);
    if old_path.exists() && !new_path.exists() {
        std::fs::rename(&old_path, &new_path).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

pub fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content).map_err(|e| format!("Write tmp failed: {}", e))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Atomic rename failed: {}", e))
}
