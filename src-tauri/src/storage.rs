use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use tauri::Manager;

pub const HERMES_DB_FILENAME: &str = "hermes_memory.db";
pub const WRITER_MEMORY_DB_FILENAME: &str = "writer_memory.db";
const MAX_FILE_BACKUPS: usize = 20;

const HERMES_DIAGNOSTIC_TABLES: &[&str] = &[
    "session_history",
    "user_drift_profile",
    "hierarchical_summaries",
    "agent_skills",
    "character_state",
    "plot_thread",
    "world_rule",
];

const WRITER_MEMORY_DIAGNOSTIC_TABLES: &[&str] = &[
    "canon_entities",
    "canon_facts",
    "plot_promises",
    "style_preferences",
    "creative_decisions",
    "proposal_feedback",
    "memory_audit_events",
    "writer_observation_trace",
    "writer_proposal_trace",
    "writer_feedback_trace",
];

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStorageDiagnostics {
    pub project_id: String,
    pub project_name: String,
    pub app_data_dir: String,
    pub project_data_dir: String,
    pub checked_at: u64,
    pub healthy: bool,
    pub files: Vec<StorageFileDiagnostic>,
    pub databases: Vec<SqliteDatabaseDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileDiagnostic {
    pub label: String,
    pub path: String,
    pub exists: bool,
    pub bytes: Option<u64>,
    pub record_count: Option<usize>,
    pub backup_count: usize,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqliteDatabaseDiagnostic {
    pub label: String,
    pub path: String,
    pub exists: bool,
    pub bytes: Option<u64>,
    pub user_version: Option<i64>,
    pub quick_check: Option<String>,
    pub table_counts: Vec<SqliteTableCount>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqliteTableCount {
    pub table: String,
    pub rows: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackupTarget {
    Lorebook,
    Outline,
    ProjectBrain,
    Chapter { title: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBackupInfo {
    pub id: String,
    pub filename: String,
    pub path: String,
    pub bytes: u64,
    pub modified_at: u64,
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

fn parse_json_list<T: for<'de> Deserialize<'de>>(
    data: &str,
    path: &std::path::Path,
    label: &str,
) -> Result<Vec<T>, String> {
    serde_json::from_str(data)
        .map_err(|e| format!("Failed to parse {} at '{}': {}", label, path.display(), e))
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
    parse_json_list(&data, &path, "lorebook")
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
    let mut safe = String::new();
    let mut last_was_dash = false;
    for ch in title.trim().to_lowercase().chars() {
        let next = if ch.is_ascii_alphanumeric() || is_cjk(ch) {
            Some(ch)
        } else if ch == ' ' || ch == '-' || ch == '_' {
            Some('-')
        } else {
            None
        };

        if let Some(ch) = next {
            if ch == '-' {
                if last_was_dash {
                    continue;
                }
                last_was_dash = true;
            } else {
                last_was_dash = false;
            }
            safe.push(ch);
        }
    }

    let safe = safe.trim_matches('-');
    let stem = if safe.is_empty() { "untitled" } else { safe };
    format!("{}.md", stem)
}

pub fn chapter_path(app: &tauri::AppHandle, title: &str) -> Result<std::path::PathBuf, String> {
    Ok(project_dir(app)?.join(chapter_filename(title)))
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
    )
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
    parse_json_list(&data, &path, "outline")
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

pub fn project_storage_diagnostics(
    app: &tauri::AppHandle,
) -> Result<ProjectStorageDiagnostics, String> {
    let manifest = active_project_manifest(app)?;
    let app_data_dir = app_data_dir(app)?;
    let project_data_dir = active_project_data_dir(app)?;
    let chapters_dir = project_dir(app)?;

    let files = vec![
        diagnose_json_array_file::<LoreEntry>("lorebook", &lorebook_path(app)?),
        diagnose_json_array_file::<OutlineNode>("outline", &outline_path(app)?),
        diagnose_json_array_file::<agent_harness_core::vector_db::Chunk>(
            "project_brain",
            &brain_path(app)?,
        ),
        diagnose_chapters_directory(&chapters_dir),
    ];
    let databases = vec![
        diagnose_sqlite_database(
            "hermes_memory",
            &project_data_dir.join(HERMES_DB_FILENAME),
            HERMES_DIAGNOSTIC_TABLES,
        ),
        diagnose_sqlite_database(
            "writer_memory",
            &project_data_dir.join(WRITER_MEMORY_DB_FILENAME),
            WRITER_MEMORY_DIAGNOSTIC_TABLES,
        ),
    ];
    let healthy = files.iter().all(|file| file.status != "error")
        && databases.iter().all(|db| db.status == "ok");

    Ok(ProjectStorageDiagnostics {
        project_id: manifest.id,
        project_name: manifest.name,
        app_data_dir: app_data_dir.to_string_lossy().to_string(),
        project_data_dir: project_data_dir.to_string_lossy().to_string(),
        checked_at: unix_time_ms(),
        healthy,
        files,
        databases,
    })
}

pub fn list_file_backups(
    app: &tauri::AppHandle,
    target: BackupTarget,
) -> Result<Vec<FileBackupInfo>, String> {
    let target_path = backup_target_path(app, &target)?;
    let backup_dir = backup_dir_for(&target_path)?;
    if !backup_dir.exists() {
        return Ok(vec![]);
    }
    let mut backups = std::fs::read_dir(&backup_dir)
        .map_err(|e| {
            format!(
                "Failed to read backup dir '{}': {}",
                backup_dir.display(),
                e
            )
        })?
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| backup_info(entry).ok())
        .collect::<Vec<_>>();
    backups.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(backups)
}

pub fn restore_file_backup(
    app: &tauri::AppHandle,
    target: BackupTarget,
    backup_id: String,
) -> Result<(), String> {
    let target_path = backup_target_path(app, &target)?;
    let backup_dir = backup_dir_for(&target_path)?;
    let backup_path = safe_backup_file_path(&backup_dir, &backup_id)?;
    if !backup_path.exists() {
        return Err(format!("Backup '{}' not found", backup_id));
    }
    let content = std::fs::read_to_string(&backup_path)
        .map_err(|e| format!("Failed to read backup '{}': {}", backup_path.display(), e))?;
    validate_backup_content(&target, &content, &backup_path)?;
    atomic_write(&target_path, &content)
}

fn backup_target_path(
    app: &tauri::AppHandle,
    target: &BackupTarget,
) -> Result<std::path::PathBuf, String> {
    match target {
        BackupTarget::Lorebook => lorebook_path(app),
        BackupTarget::Outline => outline_path(app),
        BackupTarget::ProjectBrain => brain_path(app),
        BackupTarget::Chapter { title } => chapter_path(app, title),
    }
}

fn validate_backup_content(
    target: &BackupTarget,
    content: &str,
    backup_path: &std::path::Path,
) -> Result<(), String> {
    match target {
        BackupTarget::Lorebook => serde_json::from_str::<Vec<LoreEntry>>(content)
            .map(|_| ())
            .map_err(|e| format!("Invalid lorebook backup '{}': {}", backup_path.display(), e)),
        BackupTarget::Outline => serde_json::from_str::<Vec<OutlineNode>>(content)
            .map(|_| ())
            .map_err(|e| format!("Invalid outline backup '{}': {}", backup_path.display(), e)),
        BackupTarget::ProjectBrain => {
            serde_json::from_str::<Vec<agent_harness_core::vector_db::Chunk>>(content)
                .map(|_| ())
                .map_err(|e| {
                    format!(
                        "Invalid project brain backup '{}': {}",
                        backup_path.display(),
                        e
                    )
                })
        }
        BackupTarget::Chapter { .. } => Ok(()),
    }
}

fn diagnose_json_array_file<T: for<'de> Deserialize<'de>>(
    label: &str,
    path: &std::path::Path,
) -> StorageFileDiagnostic {
    let mut diagnostic = base_file_diagnostic(label, path);
    if !diagnostic.exists {
        diagnostic.status = "missing".to_string();
        return diagnostic;
    }

    match std::fs::read_to_string(path) {
        Ok(data) => match serde_json::from_str::<Vec<T>>(&data) {
            Ok(rows) => {
                diagnostic.record_count = Some(rows.len());
                diagnostic.status = "ok".to_string();
            }
            Err(e) => {
                diagnostic.status = "error".to_string();
                diagnostic.error = Some(format!("JSON parse failed: {}", e));
            }
        },
        Err(e) => {
            diagnostic.status = "error".to_string();
            diagnostic.error = Some(format!("Read failed: {}", e));
        }
    }

    diagnostic
}

fn diagnose_chapters_directory(path: &std::path::Path) -> StorageFileDiagnostic {
    let mut diagnostic = base_file_diagnostic("chapters", path);
    if !diagnostic.exists {
        diagnostic.status = "missing".to_string();
        return diagnostic;
    }
    if !path.is_dir() {
        diagnostic.status = "error".to_string();
        diagnostic.error = Some("Expected a directory".to_string());
        return diagnostic;
    }

    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut count = 0usize;
            let mut bytes = 0u64;
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.extension().map(|e| e == "md").unwrap_or(false) {
                    count += 1;
                    if let Ok(metadata) = entry.metadata() {
                        bytes = bytes.saturating_add(metadata.len());
                    }
                }
            }
            diagnostic.record_count = Some(count);
            diagnostic.bytes = Some(bytes);
            diagnostic.status = "ok".to_string();
        }
        Err(e) => {
            diagnostic.status = "error".to_string();
            diagnostic.error = Some(format!("Read directory failed: {}", e));
        }
    }

    diagnostic
}

fn diagnose_sqlite_database(
    label: &str,
    path: &std::path::Path,
    tables: &[&str],
) -> SqliteDatabaseDiagnostic {
    let mut diagnostic = SqliteDatabaseDiagnostic {
        label: label.to_string(),
        path: path.to_string_lossy().to_string(),
        exists: path.exists(),
        bytes: file_size(path),
        user_version: None,
        quick_check: None,
        table_counts: vec![],
        status: "unknown".to_string(),
        error: None,
    };
    if !diagnostic.exists {
        diagnostic.status = "missing".to_string();
        diagnostic.error = Some("Database file is missing".to_string());
        return diagnostic;
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY;
    let conn = match Connection::open_with_flags(path, flags) {
        Ok(conn) => conn,
        Err(e) => {
            diagnostic.status = "error".to_string();
            diagnostic.error = Some(format!("Open failed: {}", e));
            return diagnostic;
        }
    };

    diagnostic.user_version = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .ok();
    match conn.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0)) {
        Ok(result) => diagnostic.quick_check = Some(result),
        Err(e) => {
            diagnostic.status = "error".to_string();
            diagnostic.error = Some(format!("quick_check failed: {}", e));
            return diagnostic;
        }
    }

    for table in tables {
        match sqlite_table_row_count(&conn, table) {
            Ok(Some(rows)) => diagnostic.table_counts.push(SqliteTableCount {
                table: (*table).to_string(),
                rows,
            }),
            Ok(None) => {}
            Err(e) => {
                diagnostic.status = "error".to_string();
                diagnostic.error = Some(format!("Count failed for '{}': {}", table, e));
                return diagnostic;
            }
        }
    }

    if diagnostic.quick_check.as_deref() == Some("ok") {
        diagnostic.status = "ok".to_string();
    } else {
        diagnostic.status = "error".to_string();
        diagnostic.error = diagnostic
            .quick_check
            .as_ref()
            .map(|check| format!("SQLite quick_check reported '{}'", check));
    }
    diagnostic
}

fn sqlite_table_row_count(conn: &Connection, table: &str) -> rusqlite::Result<Option<u64>> {
    if !sqlite_table_exists(conn, table)? {
        return Ok(None);
    }
    let count: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })?;
    Ok(Some(count.max(0) as u64))
}

fn sqlite_table_exists(conn: &Connection, table: &str) -> rusqlite::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        rusqlite::params![table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn base_file_diagnostic(label: &str, path: &std::path::Path) -> StorageFileDiagnostic {
    StorageFileDiagnostic {
        label: label.to_string(),
        path: path.to_string_lossy().to_string(),
        exists: path.exists(),
        bytes: file_size(path),
        record_count: None,
        backup_count: backup_count(path),
        status: "unknown".to_string(),
        error: None,
    }
}

fn file_size(path: &std::path::Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|metadata| metadata.len())
}

fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn rename_chapter_file(
    app: &tauri::AppHandle,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let dir = project_dir(app)?;
    let old_path = safe_chapter_file_path(&dir, &old_name)?;
    let new_path = safe_chapter_file_path(&dir, &new_name)?;
    if old_path.exists() && !new_path.exists() {
        std::fs::rename(&old_path, &new_path).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

fn safe_chapter_file_path(
    dir: &std::path::Path,
    filename: &str,
) -> Result<std::path::PathBuf, String> {
    let path = std::path::Path::new(filename);
    if path.components().count() != 1 || path.file_name().is_none() {
        return Err(format!("Invalid chapter filename: {}", filename));
    }
    if path.extension().map(|e| e != "md").unwrap_or(true) {
        return Err(format!("Chapter filename must end with .md: {}", filename));
    }
    Ok(dir.join(path))
}

pub fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), String> {
    backup_existing_file(path)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content).map_err(|e| format!("Write tmp failed: {}", e))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Atomic rename failed: {}", e))
}

fn backup_existing_file(path: &std::path::Path) -> Result<(), String> {
    if !path.exists() || path.is_dir() {
        return Ok(());
    }

    let backup_dir = backup_dir_for(path)?;
    std::fs::create_dir_all(&backup_dir).map_err(|e| {
        format!(
            "Failed to create backup dir '{}': {}",
            backup_dir.display(),
            e
        )
    })?;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let backup_path = backup_dir.join(format!("{}-{}", unix_time_ms(), filename));
    std::fs::copy(path, &backup_path).map_err(|e| {
        format!(
            "Failed to backup '{}' to '{}': {}",
            path.display(),
            backup_path.display(),
            e
        )
    })?;
    prune_backups(&backup_dir, MAX_FILE_BACKUPS)
}

fn backup_count(path: &std::path::Path) -> usize {
    let Ok(dir) = backup_dir_for(path) else {
        return 0;
    };
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().is_file())
                .count()
        })
        .unwrap_or(0)
}

fn backup_dir_for(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Path '{}' has no parent directory", path.display()))?;
    let file_stem = path
        .file_name()
        .ok_or_else(|| format!("Path '{}' has no filename", path.display()))?
        .to_string_lossy()
        .to_string();
    Ok(parent
        .join(".backups")
        .join(safe_backup_segment(&file_stem)))
}

fn prune_backups(dir: &std::path::Path, keep: usize) -> Result<(), String> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read backup dir '{}': {}", dir.display(), e))?
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in entries.into_iter().skip(keep) {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Failed to prune backup '{}': {}", path.display(), e))?;
    }
    Ok(())
}

fn safe_backup_segment(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn safe_backup_file_path(
    backup_dir: &std::path::Path,
    backup_id: &str,
) -> Result<std::path::PathBuf, String> {
    let path = std::path::Path::new(backup_id);
    if path.components().count() != 1 || path.file_name().is_none() {
        return Err(format!("Invalid backup id: {}", backup_id));
    }
    Ok(backup_dir.join(path))
}

fn backup_info(entry: std::fs::DirEntry) -> Result<FileBackupInfo, String> {
    let path = entry.path();
    let metadata = entry
        .metadata()
        .map_err(|e| format!("Failed to read backup metadata '{}': {}", path.display(), e))?;
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let filename = entry.file_name().to_string_lossy().to_string();
    Ok(FileBackupInfo {
        id: filename.clone(),
        filename,
        path: path.to_string_lossy().to_string(),
        bytes: metadata.len(),
        modified_at,
    })
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

    #[test]
    fn parse_json_list_reports_corrupt_lorebook() {
        let path = std::path::Path::new("lorebook.json");
        let err = parse_json_list::<LoreEntry>("{not json", path, "lorebook").unwrap_err();

        assert!(err.contains("Failed to parse lorebook"));
        assert!(err.contains("lorebook.json"));
    }

    #[test]
    fn parse_json_list_accepts_valid_outline() {
        let path = std::path::Path::new("outline.json");
        let nodes = parse_json_list::<OutlineNode>(
            r#"[{"chapter_title":"第一章","summary":"开端","status":"draft"}]"#,
            path,
            "outline",
        )
        .unwrap();

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].chapter_title, "第一章");
    }

    #[test]
    fn chapter_filename_rejects_path_segments() {
        assert_eq!(
            chapter_filename("../第一章: 开端/草稿"),
            "第一章-开端草稿.md"
        );
        assert_eq!(chapter_filename("   "), "untitled.md");
        assert_eq!(
            chapter_filename("Chapter 01 - Setup"),
            "chapter-01-setup.md"
        );
    }

    #[test]
    fn safe_chapter_file_path_rejects_traversal() {
        let dir = std::path::Path::new("chapters");

        assert!(safe_chapter_file_path(dir, "chapter-1.md").is_ok());
        assert!(safe_chapter_file_path(dir, "../chapter-1.md").is_err());
        assert!(safe_chapter_file_path(dir, "nested/chapter-1.md").is_err());
        assert!(safe_chapter_file_path(dir, "chapter-1.txt").is_err());
    }

    #[test]
    fn safe_backup_file_path_rejects_traversal() {
        let dir = std::path::Path::new("backups");

        assert!(safe_backup_file_path(dir, "123-chapter.md").is_ok());
        assert!(safe_backup_file_path(dir, "../123-chapter.md").is_err());
        assert!(safe_backup_file_path(dir, "nested/123-chapter.md").is_err());
    }

    #[test]
    fn diagnose_json_array_file_reports_parse_errors() {
        let path = temp_path("bad-lorebook.json");
        std::fs::write(&path, "{not json").unwrap();

        let diagnostic = diagnose_json_array_file::<LoreEntry>("lorebook", &path);

        assert_eq!(diagnostic.status, "error");
        assert!(diagnostic.error.unwrap().contains("JSON parse failed"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn diagnose_sqlite_database_reports_version_and_counts() {
        let path = temp_path("writer-memory.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "PRAGMA user_version = 7;
                CREATE TABLE canon_entities (id INTEGER PRIMARY KEY, name TEXT);
                INSERT INTO canon_entities (name) VALUES ('林墨'), ('张三');",
            )
            .unwrap();
        }

        let diagnostic =
            diagnose_sqlite_database("writer_memory", &path, &["canon_entities", "missing_table"]);

        assert_eq!(diagnostic.status, "ok");
        assert_eq!(diagnostic.user_version, Some(7));
        assert_eq!(diagnostic.quick_check.as_deref(), Some("ok"));
        assert_eq!(diagnostic.table_counts[0].table, "canon_entities");
        assert_eq!(diagnostic.table_counts[0].rows, 2);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn atomic_write_creates_bounded_backups_for_existing_files() {
        let path = temp_path("chapter.md");
        std::fs::write(&path, "old").unwrap();

        atomic_write(&path, "new").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        let backup_dir = backup_dir_for(&path).unwrap();
        let backups = std::fs::read_dir(&backup_dir)
            .unwrap()
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read_to_string(backups[0].path()).unwrap(), "old");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(backup_dir.parent().unwrap());
    }

    #[test]
    fn backup_info_lists_safe_restore_candidates() {
        let path = temp_path("outline.json");
        std::fs::write(&path, "v1").unwrap();
        atomic_write(&path, "v2").unwrap();
        let dir = backup_dir_for(&path).unwrap();
        let backups = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter_map(|entry| backup_info(entry).ok())
            .collect::<Vec<_>>();

        assert_eq!(backups.len(), 1);
        assert!(backups[0].id.ends_with("outline.json"));
        assert_eq!(backups[0].bytes, 2);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn validate_backup_content_rejects_corrupt_json_targets() {
        let path = std::path::Path::new("outline.json");

        assert!(validate_backup_content(&BackupTarget::Outline, "{bad", path).is_err());
        assert!(validate_backup_content(
            &BackupTarget::Chapter {
                title: "Chapter-1".to_string(),
            },
            "{bad",
            path,
        )
        .is_ok());
    }

    #[test]
    fn prune_backups_keeps_newest_entries() {
        let dir = std::env::temp_dir().join(format!(
            "forge-storage-test-{}-{}-backups",
            std::process::id(),
            unix_time_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..5 {
            std::fs::write(dir.join(format!("{}.txt", i)), i.to_string()).unwrap();
        }

        prune_backups(&dir, 2).unwrap();

        let count = std::fs::read_dir(&dir).unwrap().flatten().count();
        assert_eq!(count, 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let unique = unix_time_ms();
        std::env::temp_dir().join(format!(
            "forge-storage-test-{}-{}-{}",
            std::process::id(),
            unique,
            name
        ))
    }
}
