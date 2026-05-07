use super::memory::WriterMemory;

pub struct TypedFilterResult {
    pub entity_boost: f64,
    pub knowledge_boost: f64,
    pub scene_boost: f64,
    pub reasons: Vec<String>,
}

impl TypedFilterResult {
    pub fn neutral() -> Self {
        Self { entity_boost: 1.0, knowledge_boost: 1.0, scene_boost: 1.0, reasons: Vec::new() }
    }
    pub fn total_multiplier(&self) -> f64 {
        self.entity_boost * self.knowledge_boost * self.scene_boost
    }
}

pub fn apply_typed_filter(source_text: &str, chapter_title: &str, memory: &WriterMemory) -> TypedFilterResult {
    let mut result = TypedFilterResult::neutral();
    if let Ok(characters) = memory.list_characters(None) {
        for c in &characters {
            if source_text.contains(&c.name) {
                if c.role_type == "protagonist" {
                    result.entity_boost *= 1.3;
                    result.reasons.push(format!("protagonist:{}", c.name));
                }
                if let Ok(Some(state)) = memory.get_active_state(c.id, chapter_title) {
                    if let Some(commitments) = state.core_commitments.as_array() {
                        if !commitments.is_empty() {
                            result.entity_boost *= 1.15;
                            result.reasons.push(format!("pending_commitments:{}", c.name));
                        }
                    }
                }
                break;
            }
        }
    }
    if let Ok(items) = memory.list_knowledge_items(Some("objective")) {
        for item in &items {
            if source_text.contains(&item.topic) {
                result.knowledge_boost *= 1.2;
                result.reasons.push(format!("knowledge_topic:{}", item.topic));
                break;
            }
        }
    }
    if let Ok(scenes) = memory.list_scenes_by_chapter(chapter_title) {
        for scene in &scenes {
            if let Ok(Some(obl)) = memory.get_scene_obligations(scene.id) {
                if !obl.promise_ids.is_empty() || !obl.payoff_targets.is_empty() {
                    result.scene_boost *= 1.1;
                    result.reasons.push(format!("scene_obligations:{}", scene.id));
                    break;
                }
            }
        }
    }
    result
}
