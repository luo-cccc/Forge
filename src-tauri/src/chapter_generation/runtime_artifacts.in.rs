#[derive(Debug, Clone)]
pub struct PersistedChapterRuntimeArtifacts {
    pub artifact_refs: Vec<String>,
}

pub fn persist_chapter_runtime_artifacts(
    app: &tauri::AppHandle,
    request_id: &str,
    context: &BuiltChapterContext,
    settlement_delta: &ChapterSettlementDelta,
    length_telemetry: &ChapterLengthTelemetry,
) -> Result<PersistedChapterRuntimeArtifacts, String> {
    let project_dir = crate::storage::active_project_data_dir(app)?;
    let runtime_dir = project_dir.join("chapter_runtime");
    std::fs::create_dir_all(&runtime_dir).map_err(|e| e.to_string())?;

    let stem = format!(
        "{}-{}",
        context
            .target
            .number
            .map(|number| format!("chapter-{:04}", number))
            .unwrap_or_else(|| "chapter-unknown".to_string()),
        request_id
    );

    let intent_path = runtime_dir.join(format!("{}.intent.json", stem));
    let evidence_path = runtime_dir.join(format!("{}.evidence.json", stem));
    let rule_stack_path = runtime_dir.join(format!("{}.rule_stack.json", stem));
    let trace_path = runtime_dir.join(format!("{}.trace.json", stem));
    let settlement_path = runtime_dir.join(format!("{}.settlement.json", stem));
    let length_path = runtime_dir.join(format!("{}.length.json", stem));

    write_json_file(&intent_path, &context.intent_artifact)?;
    write_json_file(&evidence_path, &context.selected_evidence)?;
    write_json_file(&rule_stack_path, &context.rule_stack)?;
    write_json_file(&trace_path, &context.trace_artifact)?;
    write_json_file(&settlement_path, settlement_delta)?;
    write_json_file(&length_path, length_telemetry)?;

    Ok(PersistedChapterRuntimeArtifacts {
        artifact_refs: vec![
            path_ref(&project_dir, &intent_path),
            path_ref(&project_dir, &evidence_path),
            path_ref(&project_dir, &rule_stack_path),
            path_ref(&project_dir, &trace_path),
            path_ref(&project_dir, &settlement_path),
            path_ref(&project_dir, &length_path),
        ],
    })
}

fn write_json_file(path: &std::path::Path, value: &impl serde::Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

fn path_ref(project_dir: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(project_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
