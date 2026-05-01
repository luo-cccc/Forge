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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    pub id: String,
    pub name: String,
}

pub fn app_data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    Ok(dir)
}

fn project_manifest_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app_data_dir(app)?.join("active_project.json"))
}

fn generated_local_project_id(data_dir: &std::path::Path) -> String {
    let fingerprint = content_revision(&data_dir.to_string_lossy());
    let hash = fingerprint.split('-').next().unwrap_or("0000000000000000");
    format!("local-{}", hash)
}

fn valid_project_id(id: &str) -> bool {
    !id.trim().is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn active_project_manifest(app: &tauri::AppHandle) -> Result<ProjectManifest, String> {
    let path = project_manifest_path(app)?;
    if path.exists() {
        let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let manifest: ProjectManifest = serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse project manifest: {}", e))?;
        if !valid_project_id(&manifest.id) {
            return Err(format!("Invalid project id in manifest: {}", manifest.id));
        }
        return Ok(manifest);
    }

    let data_dir = app_data_dir(app)?;
    let manifest = ProjectManifest {
        id: generated_local_project_id(&data_dir),
        name: "Local Project".to_string(),
    };
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    atomic_write(&path, &json)?;
    Ok(manifest)
}

pub fn active_project_id(app: &tauri::AppHandle) -> Result<String, String> {
    Ok(active_project_manifest(app)?.id)
}

pub fn active_project_data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let manifest = active_project_manifest(app)?;
    let dir = app_data_dir(app)?.join("projects").join(manifest.id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create active project dir: {}", e))?;
    Ok(dir)
}

fn migrate_legacy_file_if_needed(
    target: &std::path::Path,
    legacy: &std::path::Path,
) -> Result<(), String> {
    if target.exists() || !legacy.exists() || target == legacy {
        return Ok(());
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create parent dir for '{}': {}",
                target.display(),
                e
            )
        })?;
    }
    std::fs::copy(legacy, target).map_err(|e| {
        format!(
            "Failed to migrate '{}' to '{}': {}",
            legacy.display(),
            target.display(),
            e
        )
    })?;
    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Failed to create directory '{}': {}", dst.display(), e))?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| format!("Failed to read directory '{}': {}", src.display(), e))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            if let Some(parent) = dst_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Failed to create parent dir for '{}': {}",
                        dst_path.display(),
                        e
                    )
                })?;
            }
            std::fs::copy(&src_path, &dst_path).map_err(|e| {
                format!(
                    "Failed to copy '{}' to '{}': {}",
                    src_path.display(),
                    dst_path.display(),
                    e
                )
            })?;
        }
    }
    Ok(())
}

fn migrate_legacy_dir_if_needed(
    target: &std::path::Path,
    legacy: &std::path::Path,
) -> Result<(), String> {
    if target.exists() || !legacy.exists() || target == legacy {
        return Ok(());
    }
    copy_dir_recursive(legacy, target)
}

pub fn lorebook_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let target = active_project_data_dir(app)?.join("lorebook.json");
    let legacy = app_data_dir(app)?.join("lorebook.json");
    migrate_legacy_file_if_needed(&target, &legacy)?;
    Ok(target)
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
    let dir = active_project_data_dir(app)?.join("chapters");
    let legacy = app_data_dir(app)?.join("project");
    migrate_legacy_dir_if_needed(&dir, &legacy)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create project dir: {}", e))?;
    Ok(dir)
}

pub fn chapter_filename(title: &str) -> String {
    format!("{}.md", title.replace(' ', "-").to_lowercase())
}

pub fn chapter_path(app: &tauri::AppHandle, title: &str) -> Result<std::path::PathBuf, String> {
    Ok(project_dir(app)?.join(chapter_filename(title)))
}

pub fn content_revision(content: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in content.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{:016x}-{}", hash, content.len())
}

pub fn chapter_revision(app: &tauri::AppHandle, title: &str) -> Result<String, String> {
    let path = chapter_path(app, title)?;
    if !path.exists() {
        return Ok("missing".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(content_revision(&content))
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
    let filename = chapter_filename(&title);
    let path = chapter_path(app, &title)?;
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
    let path = chapter_path(app, title)?;
    atomic_write(&path, content)
}

pub fn save_chapter_content_and_revision(
    app: &tauri::AppHandle,
    title: &str,
    content: &str,
) -> Result<String, String> {
    save_chapter_content(app, title, content)?;
    Ok(content_revision(content))
}

pub fn load_chapter(app: &tauri::AppHandle, title: String) -> Result<String, String> {
    let path = chapter_path(app, &title)?;
    if !path.exists() {
        return Err(format!("Chapter '{}' not found", title));
    }
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

pub fn outline_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let target = active_project_data_dir(app)?.join("outline.json");
    let legacy = app_data_dir(app)?.join("outline.json");
    migrate_legacy_file_if_needed(&target, &legacy)?;
    Ok(target)
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
    let target = active_project_data_dir(app)?.join("project_brain.json");
    let legacy = app_data_dir(app)?.join("project_brain.json");
    migrate_legacy_file_if_needed(&target, &legacy)?;
    Ok(target)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_local_project_id_is_stable_and_path_safe() {
        let id = generated_local_project_id(std::path::Path::new("C:/Users/Msi/AppData/Forge"));

        assert!(id.starts_with("local-"));
        assert!(valid_project_id(&id));
        assert_eq!(
            id,
            generated_local_project_id(std::path::Path::new("C:/Users/Msi/AppData/Forge"))
        );
    }

    #[test]
    fn project_id_validation_rejects_path_traversal() {
        assert!(valid_project_id("local-abc_123"));
        assert!(!valid_project_id(""));
        assert!(!valid_project_id("../novel"));
        assert!(!valid_project_id("novel/one"));
        assert!(!valid_project_id("novel one"));
    }
}
