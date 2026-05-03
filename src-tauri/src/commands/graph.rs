//! Project graph data Tauri command — entity-relationship visualization.

use crate::AppState;
use serde::Serialize;
use tauri::Manager;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GraphEntity {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) description: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GraphRelationship {
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) label: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GraphChapter {
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) status: String,
    pub(crate) word_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProjectGraphData {
    pub(crate) entities: Vec<GraphEntity>,
    pub(crate) relationships: Vec<GraphRelationship>,
    pub(crate) chapters: Vec<GraphChapter>,
}

#[tauri::command]
pub fn get_project_graph_data(app: tauri::AppHandle) -> Result<ProjectGraphData, String> {
    let mut entities = Vec::new();
    let mut relationships = Vec::new();
    let mut chapters = Vec::new();

    // 1. Entities from Lorebook
    let lore_entries = crate::storage::load_lorebook(&app)?;
    for entry in lore_entries {
        entities.push(GraphEntity {
            id: format!("lore-{}", entry.id),
            name: entry.keyword.clone(),
            category: "character".to_string(),
            description: entry.content.clone(),
        });
    }

    // 2. Entities from agent_skills (extracted character rules)
    let state = app.state::<AppState>();
    let db = crate::lock_hermes(&state)?;
    if let Ok(skills) = db.get_active_skills() {
        for skill in skills {
            if skill.category == "character" {
                let name = skill.skill.chars().take(30).collect::<String>();
                entities.push(GraphEntity {
                    id: format!("skill-{}", skill.id),
                    name,
                    category: "character_trait".to_string(),
                    description: skill.skill.clone(),
                });
            }
        }
    }
    drop(db);

    // 3. Chapters from file tree + outline
    let dir = crate::storage::project_dir(&app)?;
    match crate::storage::load_outline(&app) {
        Ok(outline_nodes) => {
            for node in outline_nodes {
                let filename =
                    format!("{}.md", node.chapter_title.replace(' ', "-").to_lowercase());
                let path = dir.join(&filename);
                let word_count = if path.exists() {
                    std::fs::read_to_string(&path)
                        .map(|s| s.split_whitespace().count())
                        .unwrap_or(0)
                } else {
                    0
                };
                chapters.push(GraphChapter {
                    title: node.chapter_title.clone(),
                    summary: node.summary.clone(),
                    status: node.status.clone(),
                    word_count,
                });
            }
        }
        Err(e) => {
            tracing::warn!(
                "Project graph skipped outline because it failed to load: {}",
                e
            );
        }
    }

    // If outline is empty, derive chapters from file tree
    if chapters.is_empty() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    let title = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    let word_count = content.split_whitespace().count();
                    chapters.push(GraphChapter {
                        title,
                        summary: String::new(),
                        status: "empty".to_string(),
                        word_count,
                    });
                }
            }
        }
    }

    // 4. Relationships: co-occurrence of entities in same chapter
    let entity_names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
    for chapter in &chapters {
        let filename = format!("{}.md", chapter.title.replace(' ', "-").to_lowercase());
        let path = dir.join(&filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            let content_lower = content.to_lowercase();
            let found: Vec<&String> = entity_names
                .iter()
                .filter(|name| content_lower.contains(&name.to_lowercase()))
                .collect();
            if found.len() >= 2 {
                for i in 0..found.len() {
                    for j in i + 1..found.len() {
                        let exists = relationships.iter().any(|r: &GraphRelationship| {
                            (r.source == *found[i] && r.target == *found[j])
                                || (r.source == *found[j] && r.target == *found[i])
                        });
                        if !exists {
                            relationships.push(GraphRelationship {
                                source: found[i].clone(),
                                target: found[j].clone(),
                                label: format!("Co-occur in {}", chapter.title),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(ProjectGraphData {
        entities,
        relationships,
        chapters,
    })
}
