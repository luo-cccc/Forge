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

include!("writing_relevance/part_a.in.rs");
include!("writing_relevance/part_b.in.rs");
