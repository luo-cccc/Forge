#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_harness_core::{Chunk, VectorDB};
use agent_writer_lib::brain_service::{
    build_project_brain_knowledge_index, compare_project_brain_source_revisions_from_db,
    project_brain_embedding_batch_status, project_brain_embedding_profile_from_config,
    project_brain_embedding_provider_registry, project_brain_source_revision,
    rerank_project_brain_results_with_focus, resolve_project_brain_embedding_profile,
    restore_project_brain_source_revision_in_db, safe_knowledge_index_file_path,
    search_project_brain_results_with_focus, trim_embedding_input,
    ProjectBrainEmbeddingBatchStatus, ProjectBrainEmbeddingRegistryStatus, ProjectBrainFocus,
};
use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::context_relevance::{
    format_text_chunk_relevance, rerank_text_chunks, writing_scene_types,
};
use agent_writer_lib::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::observation::{ObservationReason, ObservationSource};
use agent_writer_lib::writer_agent::operation::WriterOperation;
use agent_writer_lib::writer_agent::proposal::ProposalKind;
use agent_writer_lib::writer_agent::WriterAgentKernel;

pub fn run_project_brain_knowledge_index_graph_eval() -> EvalResult {
    let mut db = VectorDB::new();
    db.upsert(Chunk {
        id: "chunk-ring-payoff".to_string(),
        chapter: "Chapter-5".to_string(),
        text: "林墨在霜铃塔发现寒玉戒指的裂纹，与张三隐瞒的旧门钥匙有关。".to_string(),
        embedding: vec![1.0, 0.0],
        keywords: vec![
            "寒玉戒指".to_string(),
            "霜铃塔".to_string(),
            "旧门钥匙".to_string(),
        ],
        topic: Some("寒玉戒指下落".to_string()),
        source_ref: None,
        source_revision: None,
        source_kind: None,
        chunk_index: None,
        archived: false,
    });
    let outline = vec![agent_writer_lib::brain_service::OutlineNode {
        chapter_title: "Chapter-5".to_string(),
        summary: "林墨前往霜铃塔，追查寒玉戒指和旧门钥匙的关系。".to_string(),
        status: "draft".to_string(),
    }];
    let lorebook = vec![agent_writer_lib::brain_service::LoreEntry {
        id: "ring".to_string(),
        keyword: "寒玉戒指".to_string(),
        content: "寒玉戒指是林墨母亲留下的遗物，裂纹会在霜铃塔附近显现。".to_string(),
    }];
    let index = build_project_brain_knowledge_index("eval", &db, &outline, &lorebook);

    let mut errors = Vec::new();
    for kind in ["lore", "outline", "chunk"] {
        if !index.nodes.iter().any(|node| node.kind == kind) {
            errors.push(format!("knowledge index missing {} node", kind));
        }
    }
    if !index.nodes.iter().any(|node| {
        node.source_ref == "lorebook:ring" && node.keywords.iter().any(|kw| kw == "寒玉戒指")
    }) {
        errors.push("lore node lacks source ref or keyword".to_string());
    }
    if !index.edges.iter().any(|edge| {
        edge.relation.contains("寒玉戒指")
            && index
                .nodes
                .iter()
                .any(|node| node.id == edge.from && node.kind == "lore")
            && index
                .nodes
                .iter()
                .any(|node| node.id == edge.to && node.kind != "lore")
    }) {
        errors.push(
            "knowledge graph lacks shared keyword edge from lore to project sources".to_string(),
        );
    }
    if index.source_count != 3 {
        errors.push(format!(
            "source count should be 3, got {}",
            index.source_count
        ));
    }

    eval_result(
        "writer_agent:project_brain_knowledge_index_graph",
        format!("nodes={} edges={}", index.nodes.len(), index.edges.len()),
        errors,
    )
}

pub fn run_project_brain_knowledge_index_path_guard_eval() -> EvalResult {
    let root = std::env::temp_dir().join(format!("forge-knowledge-index-{}", std::process::id()));
    let _ = std::fs::create_dir_all(root.join("notes"));
    let mut errors = Vec::new();

    if safe_knowledge_index_file_path(&root, "notes/index.md").is_err() {
        errors.push("safe relative knowledge path was rejected".to_string());
    }
    for unsafe_path in ["../secret.md", "notes/../../secret.md"] {
        if safe_knowledge_index_file_path(&root, unsafe_path).is_ok() {
            errors.push(format!(
                "unsafe knowledge path was accepted: {}",
                unsafe_path
            ));
        }
    }
    if safe_knowledge_index_file_path(&root, "C:/Windows/system32/drivers/etc/hosts").is_ok() {
        errors.push("absolute knowledge path was accepted".to_string());
    }
    let _ = std::fs::remove_dir_all(&root);

    eval_result(
        "writer_agent:project_brain_knowledge_index_path_guard",
        format!("root={}", root.display()),
        errors,
    )
}

pub fn run_project_brain_chunk_source_version_eval() -> EvalResult {
    let chapter_text = "林墨在霜铃塔发现寒玉戒指的裂纹。\n\n张三承认旧门钥匙来自同一宗门。";
    let revision = project_brain_source_revision(chapter_text);
    let older_revision = project_brain_source_revision("旧版：林墨只追查霜铃塔传闻。");
    let mut db = VectorDB::new();
    db.upsert(Chunk {
        id: "chapter-5-old-0".to_string(),
        chapter: "Chapter-5".to_string(),
        text: "旧版中林墨只追查霜铃塔传闻，没有把寒玉戒指和祭图关联起来。".to_string(),
        embedding: vec![0.5, 0.5],
        keywords: vec!["霜铃塔传闻".to_string(), "旧版线索".to_string()],
        topic: Some("旧版霜铃塔传闻".to_string()),
        source_ref: Some("chapter:Chapter-5".to_string()),
        source_revision: Some(older_revision.clone()),
        source_kind: Some("chapter".to_string()),
        chunk_index: Some(0),
        archived: true,
    });
    db.upsert(Chunk {
        id: "chapter-5-0".to_string(),
        chapter: "Chapter-5".to_string(),
        text: chapter_text.to_string(),
        embedding: vec![1.0, 0.0],
        keywords: vec!["寒玉戒指".to_string(), "旧门钥匙".to_string()],
        topic: Some("寒玉戒指来源".to_string()),
        source_ref: Some("chapter:Chapter-5".to_string()),
        source_revision: Some(revision.clone()),
        source_kind: Some("chapter".to_string()),
        chunk_index: Some(0),
        archived: false,
    });
    db.upsert(Chunk {
        id: "chapter-5-1".to_string(),
        chapter: "Chapter-5".to_string(),
        text: "林墨把寒玉戒指和旧门钥匙放在同一张祭图上。".to_string(),
        embedding: vec![0.0, 1.0],
        keywords: vec!["寒玉戒指".to_string(), "祭图".to_string()],
        topic: Some("寒玉戒指复核".to_string()),
        source_ref: Some("chapter:Chapter-5".to_string()),
        source_revision: Some(revision.clone()),
        source_kind: Some("chapter".to_string()),
        chunk_index: Some(1),
        archived: false,
    });
    let index = build_project_brain_knowledge_index("eval", &db, &[], &[]);
    let node = index.nodes.iter().find(|node| {
        node.label == "Chapter-5" && node.source_revision.as_deref() == Some(revision.as_str())
    });
    let source_history = index
        .source_history
        .iter()
        .find(|source| source.source_ref == "chapter:Chapter-5");
    let compare = compare_project_brain_source_revisions_from_db("chapter:Chapter-5", &db);

    let mut errors = Vec::new();
    let active_chunk = db.chunks.iter().find(|chunk| chunk.id == "chapter-5-0");
    if active_chunk.and_then(|chunk| chunk.source_ref.as_deref()) != Some("chapter:Chapter-5") {
        errors.push(format!(
            "chunk source_ref mismatch: {:?}",
            active_chunk.and_then(|chunk| chunk.source_ref.as_deref())
        ));
    }
    if active_chunk.and_then(|chunk| chunk.source_revision.as_deref()) != Some(revision.as_str()) {
        errors.push(format!(
            "chunk source_revision mismatch: {:?}",
            active_chunk.and_then(|chunk| chunk.source_revision.as_deref())
        ));
    }
    if active_chunk.and_then(|chunk| chunk.source_kind.as_deref()) != Some("chapter")
        || active_chunk.and_then(|chunk| chunk.chunk_index) != Some(0)
    {
        errors.push(format!(
            "chunk source kind/index mismatch: {:?} {:?}",
            active_chunk.and_then(|chunk| chunk.source_kind.as_deref()),
            active_chunk.and_then(|chunk| chunk.chunk_index)
        ));
    }
    match node {
        Some(node) => {
            if node.kind != "chunk"
                || node.source_ref != "chapter:Chapter-5"
                || node.source_revision.as_deref() != Some(revision.as_str())
                || node.source_kind.as_deref() != Some("chapter")
                || node.chunk_index != Some(0)
            {
                errors.push(format!(
                    "knowledge node should preserve chunk source metadata, got kind={} source={} revision={:?} sourceKind={:?} chunkIndex={:?}",
                    node.kind,
                    node.source_ref,
                    node.source_revision,
                    node.source_kind,
                    node.chunk_index
                ));
            }
        }
        None => errors.push("knowledge index missing sourced chunk node".to_string()),
    }
    match source_history {
        Some(history) => {
            if history.source_kind != "chapter"
                || history.node_count != 3
                || history.chunk_count != 3
                || history.revisions.len() != 2
            {
                errors.push(format!(
                    "source history aggregation mismatch: kind={} nodes={} chunks={} revisions={}",
                    history.source_kind,
                    history.node_count,
                    history.chunk_count,
                    history.revisions.len()
                ));
            }
            if let Some(history_revision) = history
                .revisions
                .iter()
                .find(|history_revision| history_revision.revision == revision)
            {
                if history_revision.revision != revision
                    || history_revision.node_count != 2
                    || history_revision.chunk_indexes != vec![0, 1]
                    || !history_revision.active
                {
                    errors.push(format!(
                        "source revision history mismatch: revision={} nodes={} chunks={:?} active={}",
                        history_revision.revision,
                        history_revision.node_count,
                        history_revision.chunk_indexes,
                        history_revision.active
                    ));
                }
            } else {
                errors.push("source history missing revision entry".to_string());
            }
            if !history.revisions.iter().any(|history_revision| {
                history_revision.revision == older_revision && !history_revision.active
            }) {
                errors.push("source history missing archived revision entry".to_string());
            }
        }
        None => errors.push("knowledge index missing source history".to_string()),
    }
    if compare.active_revision.as_deref() != Some(revision.as_str())
        || compare.revisions.len() != 2
        || !compare
            .added_keywords
            .iter()
            .any(|keyword| keyword == "祭图")
        || !compare
            .removed_keywords
            .iter()
            .any(|keyword| keyword == "旧版线索")
    {
        errors.push(format!(
            "source compare mismatch: active={:?} revisions={} added={:?} removed={:?}",
            compare.active_revision,
            compare.revisions.len(),
            compare.added_keywords,
            compare.removed_keywords
        ));
    }

    eval_result(
        "writer_agent:project_brain_chunk_source_version",
        format!(
            "source={:?} revision={:?} nodeKind={} sourceKind={} historySources={}",
            active_chunk.and_then(|chunk| chunk.source_ref.as_ref()),
            active_chunk.and_then(|chunk| chunk.source_revision.as_ref()),
            node.map(|node| node.kind.as_str()).unwrap_or("none"),
            node.and_then(|node| node.source_kind.as_deref())
                .unwrap_or("none"),
            index.source_history.len()
        ),
        errors,
    )
}

pub fn run_project_brain_source_revision_restore_eval() -> EvalResult {
    let active_revision = project_brain_source_revision("新版：寒玉戒指已经和旧门钥匙合流。");
    let archived_revision = project_brain_source_revision("旧版：寒玉戒指仍留在霜铃塔。");
    let other_revision = project_brain_source_revision("旁支：潮汐祭账继续独立推进。");
    let mut db = VectorDB::new();
    db.upsert(Chunk {
        id: "chapter-5-old-0".to_string(),
        chapter: "Chapter-5".to_string(),
        text: "旧版中寒玉戒指仍留在霜铃塔，没有和旧门钥匙合流。".to_string(),
        embedding: vec![0.2, 0.8],
        keywords: vec!["霜铃塔".to_string(), "寒玉戒指".to_string()],
        topic: Some("旧版戒指位置".to_string()),
        source_ref: Some("chapter:Chapter-5".to_string()),
        source_revision: Some(archived_revision.clone()),
        source_kind: Some("chapter".to_string()),
        chunk_index: Some(0),
        archived: true,
    });
    db.upsert(Chunk {
        id: "chapter-5-new-0".to_string(),
        chapter: "Chapter-5".to_string(),
        text: "新版中寒玉戒指已经和旧门钥匙合流。".to_string(),
        embedding: vec![0.9, 0.1],
        keywords: vec!["旧门钥匙".to_string(), "寒玉戒指".to_string()],
        topic: Some("新版戒指合流".to_string()),
        source_ref: Some("chapter:Chapter-5".to_string()),
        source_revision: Some(active_revision.clone()),
        source_kind: Some("chapter".to_string()),
        chunk_index: Some(0),
        archived: false,
    });
    db.upsert(Chunk {
        id: "chapter-6-0".to_string(),
        chapter: "Chapter-6".to_string(),
        text: "潮汐祭账继续独立推进，不属于 Chapter-5 的回滚范围。".to_string(),
        embedding: vec![0.1, 0.9],
        keywords: vec!["潮汐祭账".to_string()],
        topic: Some("旁支来源".to_string()),
        source_ref: Some("chapter:Chapter-6".to_string()),
        source_revision: Some(other_revision.clone()),
        source_kind: Some("chapter".to_string()),
        chunk_index: Some(0),
        archived: false,
    });

    let restore = restore_project_brain_source_revision_in_db(
        "chapter:Chapter-5",
        &archived_revision,
        &mut db,
    );
    let compare = compare_project_brain_source_revisions_from_db("chapter:Chapter-5", &db);

    let mut errors = Vec::new();
    match restore {
        Ok(report) => {
            if report.restored_revision != archived_revision
                || report.previous_active_revisions != vec![active_revision.clone()]
                || report.changed_chunk_count != 2
                || report.active_chunk_count != 1
                || report.archived_chunk_count != 1
            {
                errors.push(format!(
                    "restore report mismatch: restored={} previous={:?} changed={} active={} archived={}",
                    report.restored_revision,
                    report.previous_active_revisions,
                    report.changed_chunk_count,
                    report.active_chunk_count,
                    report.archived_chunk_count
                ));
            }
        }
        Err(error) => errors.push(format!("restore failed: {}", error)),
    }

    let restored_chunk = db.chunks.iter().find(|chunk| chunk.id == "chapter-5-old-0");
    if restored_chunk.map(|chunk| chunk.archived) != Some(false) {
        errors.push("requested revision chunk was not activated".to_string());
    }
    let previous_chunk = db.chunks.iter().find(|chunk| chunk.id == "chapter-5-new-0");
    if previous_chunk.map(|chunk| chunk.archived) != Some(true) {
        errors.push("previous active revision was not archived".to_string());
    }
    let other_chunk = db.chunks.iter().find(|chunk| chunk.id == "chapter-6-0");
    if other_chunk.map(|chunk| chunk.archived) != Some(false)
        || other_chunk.and_then(|chunk| chunk.source_revision.as_deref())
            != Some(other_revision.as_str())
    {
        errors.push("restore changed a different source_ref".to_string());
    }
    if compare.active_revision.as_deref() != Some(archived_revision.as_str())
        || !compare
            .removed_keywords
            .iter()
            .any(|keyword| keyword == "旧门钥匙")
    {
        errors.push(format!(
            "source compare did not reflect restored revision: active={:?} removed={:?}",
            compare.active_revision, compare.removed_keywords
        ));
    }
    if restore_project_brain_source_revision_in_db("chapter:Chapter-5", "missing-rev", &mut db)
        .is_ok()
    {
        errors.push("missing revision restore was accepted".to_string());
    }
    if restore_project_brain_source_revision_in_db("chapter:Missing", &archived_revision, &mut db)
        .is_ok()
    {
        errors.push("missing source restore was accepted".to_string());
    }

    eval_result(
        "writer_agent:project_brain_source_revision_restore",
        format!(
            "active={:?} chunks={}",
            compare.active_revision,
            db.chunks.len()
        ),
        errors,
    )
}

pub fn run_project_brain_embedding_provider_limits_eval() -> EvalResult {
    let profile = project_brain_embedding_profile_from_config(
        "https://openrouter.ai/api/v1",
        "text-embedding-3-large",
        48,
    );
    let (trimmed, truncated) =
        trim_embedding_input("寒玉戒指".repeat(40).as_str(), profile.input_limit_chars);

    let mut errors = Vec::new();
    if profile.provider_id != "openrouter" {
        errors.push(format!(
            "provider id should be openrouter, got {}",
            profile.provider_id
        ));
    }
    if profile.model != "text-embedding-3-large" {
        errors.push(format!("profile model mismatch: {}", profile.model));
    }
    if profile.dimensions != 3072 {
        errors.push(format!(
            "text-embedding-3-large dimensions should be 3072, got {}",
            profile.dimensions
        ));
    }
    if profile.input_limit_chars != 48 {
        errors.push(format!(
            "input limit should come from settings, got {}",
            profile.input_limit_chars
        ));
    }
    if profile.batch_limit == 0 || profile.retry_limit == 0 {
        errors.push("profile lacks batch/retry limits".to_string());
    }
    if profile.provider_status != ProjectBrainEmbeddingRegistryStatus::RegistryKnown
        || profile.model_status != ProjectBrainEmbeddingRegistryStatus::RegistryKnown
    {
        errors.push(format!(
            "known openrouter/model should resolve through registry, got provider={:?} model={:?}",
            profile.provider_status, profile.model_status
        ));
    }
    if !truncated || trimmed.chars().count() > profile.input_limit_chars {
        errors.push(format!(
            "embedding input was not truncated to limit: truncated={} chars={}",
            truncated,
            trimmed.chars().count()
        ));
    }
    if project_brain_embedding_batch_status(3, 3, 0, &[])
        != ProjectBrainEmbeddingBatchStatus::Complete
    {
        errors.push("all embedded chunks should report a complete batch".to_string());
    }
    if project_brain_embedding_batch_status(3, 2, 1, &[])
        != ProjectBrainEmbeddingBatchStatus::Partial
    {
        errors.push("skipped chunks should report a partial batch".to_string());
    }
    if project_brain_embedding_batch_status(3, 0, 3, &[]) != ProjectBrainEmbeddingBatchStatus::Empty
    {
        errors.push("zero embedded chunks should report an empty batch".to_string());
    }

    eval_result(
        "writer_agent:project_brain_embedding_provider_limits",
        format!(
            "provider={} model={} dims={} limit={} truncated={}",
            profile.provider_id,
            profile.model,
            profile.dimensions,
            profile.input_limit_chars,
            truncated
        ),
        errors,
    )
}

pub fn run_project_brain_embedding_provider_registry_eval() -> EvalResult {
    let registry = project_brain_embedding_provider_registry();
    let openai_profile = resolve_project_brain_embedding_profile(
        "https://api.openai.com/v1",
        "text-embedding-3-small",
        None,
    );
    let local_profile = resolve_project_brain_embedding_profile(
        "http://127.0.0.1:11434/v1",
        "text-embedding-ada-002",
        None,
    );
    let fallback_profile = resolve_project_brain_embedding_profile(
        "https://embeddings.example.invalid/v1",
        "custom-embedding-model",
        None,
    );
    let override_profile = resolve_project_brain_embedding_profile(
        "https://api.openai.com/v1",
        "text-embedding-3-large",
        Some(4096),
    );

    let mut errors = Vec::new();
    if registry.providers.len() < 3 {
        errors.push(format!(
            "registry should expose openai/openrouter/local providers, got {}",
            registry.providers.len()
        ));
    }
    if !registry
        .providers
        .iter()
        .any(|provider| provider.provider_id == "openrouter")
    {
        errors.push("registry missing openrouter provider".to_string());
    }
    if openai_profile.provider_id != "openai"
        || openai_profile.provider_status != ProjectBrainEmbeddingRegistryStatus::RegistryKnown
    {
        errors.push(format!(
            "OpenAI base should resolve as known openai provider, got {} {:?}",
            openai_profile.provider_id, openai_profile.provider_status
        ));
    }
    if openai_profile.dimensions != 1536
        || openai_profile.model_status != ProjectBrainEmbeddingRegistryStatus::RegistryKnown
    {
        errors.push(format!(
            "OpenAI text-embedding-3-small should be known 1536 dims, got {} {:?}",
            openai_profile.dimensions, openai_profile.model_status
        ));
    }
    if local_profile.provider_id != "local-openai-compatible"
        || local_profile.retry_limit != 0
        || local_profile.batch_limit != 8
    {
        errors.push(format!(
            "local provider policy mismatch: provider={} batch={} retry={}",
            local_profile.provider_id, local_profile.batch_limit, local_profile.retry_limit
        ));
    }
    if fallback_profile.provider_id != "openai-compatible"
        || fallback_profile.provider_status
            != ProjectBrainEmbeddingRegistryStatus::CompatibilityFallback
        || fallback_profile.model_status
            != ProjectBrainEmbeddingRegistryStatus::CompatibilityFallback
    {
        errors.push(format!(
            "unknown provider/model should be explicit compatibility fallback, got {} {:?} {:?}",
            fallback_profile.provider_id,
            fallback_profile.provider_status,
            fallback_profile.model_status
        ));
    }
    if fallback_profile.dimensions != 1536
        || fallback_profile.batch_limit != 8
        || fallback_profile.retry_limit != 0
    {
        errors.push(format!(
            "fallback policy mismatch: dims={} batch={} retry={}",
            fallback_profile.dimensions, fallback_profile.batch_limit, fallback_profile.retry_limit
        ));
    }
    if override_profile.input_limit_chars != 4096 || override_profile.dimensions != 3072 {
        errors.push(format!(
            "profile override/model dimensions mismatch: limit={} dims={}",
            override_profile.input_limit_chars, override_profile.dimensions
        ));
    }

    eval_result(
        "writer_agent:project_brain_embedding_provider_registry",
        format!(
            "providers={} openai={} local={} fallback={} overrideLimit={}",
            registry.providers.len(),
            openai_profile.provider_id,
            local_profile.provider_id,
            fallback_profile.provider_id,
            override_profile.input_limit_chars
        ),
        errors,
    )
}
