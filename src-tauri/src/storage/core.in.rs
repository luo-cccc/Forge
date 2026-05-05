
pub const HERMES_DB_FILENAME: &str = "hermes_memory.db";
pub const WRITER_MEMORY_DB_FILENAME: &str = "writer_memory.db";
const MAX_FILE_BACKUPS: usize = 20;
static ACTIVE_WRITE_LOCKS: OnceLock<(Mutex<HashSet<PathBuf>>, Condvar)> = OnceLock::new();
static ATOMIC_WRITE_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    "story_contracts",
    "chapter_missions",
    "chapter_result_snapshots",
    "canon_entities",
    "canon_facts",
    "canon_rules",
    "plot_promises",
    "style_preferences",
    "creative_decisions",
    "proposal_feedback",
    "memory_audit_events",
    "manual_agent_turns",
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

pub fn patch_outline_node(
    app: &tauri::AppHandle,
    chapter_title: String,
    patch: serde_json::Value,
) -> Result<Vec<OutlineNode>, String> {
    let mut nodes = load_outline(app)?;
    apply_outline_patch(&mut nodes, &chapter_title, &patch)?;
    save_outline(app, &nodes)?;
    Ok(nodes)
}

fn apply_outline_patch(
    nodes: &mut [OutlineNode],
    chapter_title: &str,
    patch: &serde_json::Value,
) -> Result<(), String> {
    let patch = patch
        .as_object()
        .ok_or_else(|| "outline patch must be an object".to_string())?;
    let node = nodes
        .iter_mut()
        .find(|node| node.chapter_title == chapter_title)
        .ok_or_else(|| format!("Outline node '{}' not found", chapter_title))?;

    for (key, value) in patch {
        match key.as_str() {
            "chapterTitle" | "chapter_title" => {
                let next = value
                    .as_str()
                    .ok_or_else(|| "outline chapterTitle must be a string".to_string())?
                    .trim();
                if next.is_empty() {
                    return Err("outline chapterTitle cannot be empty".to_string());
                }
                node.chapter_title = next.to_string();
            }
            "summary" => {
                node.summary = value
                    .as_str()
                    .ok_or_else(|| "outline summary must be a string".to_string())?
                    .to_string();
            }
            "status" => {
                node.status = value
                    .as_str()
                    .ok_or_else(|| "outline status must be a string".to_string())?
                    .to_string();
            }
            other => return Err(format!("Unsupported outline patch field '{}'", other)),
        }
    }

    Ok(())
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
    backups.sort_by_key(|b| std::cmp::Reverse(b.modified_at));
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

