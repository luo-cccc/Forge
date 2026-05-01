use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
    Builtin,
    Project,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritingSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
    pub source: SkillSource,
    pub path: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLoadDiagnostic {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLoadReport {
    pub skills: Vec<WritingSkill>,
    pub diagnostics: Vec<SkillLoadDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct SkillRoot {
    pub path: PathBuf,
    pub source: SkillSource,
}

#[derive(Debug, Clone)]
pub struct SkillLoader {
    max_depth: usize,
}

impl Default for SkillLoader {
    fn default() -> Self {
        Self { max_depth: 4 }
    }
}

impl SkillLoader {
    pub fn new(max_depth: usize) -> Self {
        Self { max_depth }
    }

    pub fn load(&self, roots: &[SkillRoot]) -> SkillLoadReport {
        let mut by_id: HashMap<String, WritingSkill> = HashMap::new();
        let mut diagnostics = Vec::new();

        for root in roots {
            if !root.path.exists() {
                continue;
            }
            self.scan_dir(&root.path, &root.source, 0, &mut by_id, &mut diagnostics);
        }

        let mut skills = by_id.into_values().collect::<Vec<_>>();
        skills.sort_by(|a, b| a.name.cmp(&b.name).then(a.path.cmp(&b.path)));
        SkillLoadReport {
            skills,
            diagnostics,
        }
    }

    fn scan_dir(
        &self,
        dir: &Path,
        source: &SkillSource,
        depth: usize,
        by_id: &mut HashMap<String, WritingSkill>,
        diagnostics: &mut Vec<SkillLoadDiagnostic>,
    ) {
        if depth > self.max_depth || is_ignored_dir(dir) {
            return;
        }

        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) => {
                diagnostics.push(SkillLoadDiagnostic {
                    path: dir.display().to_string(),
                    message: format!("Failed to read skill directory: {}", error),
                });
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.scan_dir(&path, source, depth + 1, by_id, diagnostics);
            } else if is_skill_file(&path) {
                match parse_skill_file(&path, source.clone()) {
                    Ok(skill) => {
                        by_id.insert(skill.id.clone(), skill);
                    }
                    Err(message) => diagnostics.push(SkillLoadDiagnostic {
                        path: path.display().to_string(),
                        message,
                    }),
                }
            }
        }
    }
}

fn is_ignored_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            matches!(
                name,
                ".git" | ".hg" | ".svn" | "node_modules" | "target" | "dist" | ".tauri"
            )
        })
        .unwrap_or(false)
}

fn is_skill_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    file_name.eq_ignore_ascii_case("SKILL.md")
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("md"))
            .unwrap_or(false)
}

fn parse_skill_file(path: &Path, source: SkillSource) -> Result<WritingSkill, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let (metadata, body) = parse_frontmatter(&text);
    let fallback_name = fallback_skill_name(path, body);
    let name = metadata
        .get("name")
        .or_else(|| metadata.get("title"))
        .cloned()
        .unwrap_or(fallback_name);
    let id = metadata
        .get("id")
        .cloned()
        .unwrap_or_else(|| normalize_id(&name));
    let description = metadata
        .get("description")
        .cloned()
        .unwrap_or_else(|| first_body_sentence(body));
    let category = metadata
        .get("category")
        .cloned()
        .unwrap_or_else(|| "writing".to_string());
    let tags = metadata
        .get("tags")
        .map(|tags| parse_tags(tags))
        .unwrap_or_else(|| vec![category.clone()]);

    Ok(WritingSkill {
        id,
        name,
        description,
        category,
        tags,
        source,
        path: path.display().to_string(),
        body: body.trim().to_string(),
    })
}

fn parse_frontmatter(text: &str) -> (HashMap<String, String>, &str) {
    let mut metadata = HashMap::new();
    let normalized = text.strip_prefix('\u{feff}').unwrap_or(text);
    if !normalized.starts_with("---") {
        return (metadata, normalized);
    }

    let remainder = &normalized[3..];
    let Some(end) = remainder.find("\n---") else {
        return (metadata, normalized);
    };

    let frontmatter = &remainder[..end];
    let body_start = end + "\n---".len();
    let body = remainder[body_start..].trim_start_matches(['\r', '\n']);

    for line in frontmatter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        if !key.is_empty() && !value.is_empty() {
            metadata.insert(key, value);
        }
    }

    (metadata, body)
}

fn parse_tags(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|tag| tag.trim().trim_matches('"').trim_matches('\''))
        .filter(|tag| !tag.is_empty())
        .map(|tag| tag.to_string())
        .collect()
}

fn fallback_skill_name(path: &Path, body: &str) -> String {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(stripped) = trimmed.strip_prefix("# ") {
            return stripped.trim().to_string();
        }
    }

    path.parent()
        .and_then(|parent| parent.file_name())
        .or_else(|| path.file_stem())
        .and_then(|name| name.to_str())
        .unwrap_or("writing-skill")
        .to_string()
}

fn first_body_sentence(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .unwrap_or("")
        .chars()
        .take(180)
        .collect()
}

fn normalize_id(name: &str) -> String {
    let mut id = String::new();
    let mut last_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if ch.is_alphanumeric() {
            id.push(ch);
            last_dash = false;
        } else if !last_dash {
            id.push('-');
            last_dash = true;
        }
    }
    id.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("forge-skill-test-{}", suffix))
    }

    #[test]
    fn loader_parses_frontmatter_and_body() {
        let dir = unique_temp_dir();
        fs::create_dir_all(dir.join("tone")).unwrap();
        fs::write(
            dir.join("tone").join("SKILL.md"),
            "---\nname: Tension Control\ncategory: pacing\ntags: [pacing, scene]\n---\nKeep scene pressure visible.",
        )
        .unwrap();

        let report = SkillLoader::default().load(&[SkillRoot {
            path: dir.clone(),
            source: SkillSource::Project,
        }]);

        assert_eq!(report.skills.len(), 1);
        assert_eq!(report.skills[0].id, "tension-control");
        assert_eq!(report.skills[0].category, "pacing");
        assert_eq!(report.skills[0].tags, vec!["pacing", "scene"]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn later_roots_override_earlier_skills() {
        let builtin = unique_temp_dir();
        let user = unique_temp_dir();
        fs::create_dir_all(&builtin).unwrap();
        fs::create_dir_all(&user).unwrap();
        fs::write(
            builtin.join("SKILL.md"),
            "---\nid: voice\nname: Voice\n---\nBuiltin version.",
        )
        .unwrap();
        fs::write(
            user.join("SKILL.md"),
            "---\nid: voice\nname: Voice\n---\nUser version.",
        )
        .unwrap();

        let report = SkillLoader::default().load(&[
            SkillRoot {
                path: builtin.clone(),
                source: SkillSource::Builtin,
            },
            SkillRoot {
                path: user.clone(),
                source: SkillSource::User,
            },
        ]);

        assert_eq!(report.skills.len(), 1);
        assert_eq!(report.skills[0].body, "User version.");
        assert_eq!(report.skills[0].source, SkillSource::User);

        let _ = fs::remove_dir_all(builtin);
        let _ = fs::remove_dir_all(user);
    }

    #[test]
    fn normalized_id_keeps_non_ascii_names() {
        assert_eq!(normalize_id("节奏 控制"), "节奏-控制");
    }
}
