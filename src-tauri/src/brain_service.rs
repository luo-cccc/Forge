use agent_harness_core::{
    chunk_text,
    vector_db::{Chunk, VectorDB},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager};

use crate::writer_agent::context_relevance::{
    format_text_chunk_relevance, score_text_for_writing_focus,
};
use crate::writer_agent::kernel::{ModelStartedEventContext, WriterAgentKernel};
use crate::writer_agent::provider_budget::{
    apply_provider_budget_approval, evaluate_provider_budget, WriterProviderBudgetApproval,
    WriterProviderBudgetReport, WriterProviderBudgetRequest, WriterProviderBudgetTask,
};
use crate::writer_agent::task_receipt::{WriterFailureCategory, WriterFailureEvidenceBundle};
use crate::{llm_runtime, storage};

include!("brain_service/types.in.rs");
include!("brain_service/functions_a.in.rs");
include!("brain_service/functions_b.in.rs");
