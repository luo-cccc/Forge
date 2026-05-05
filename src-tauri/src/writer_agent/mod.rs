// ── Kernel (core runtime) ──
pub mod kernel;
pub mod kernel_chapters;
pub mod kernel_ghost;
pub mod kernel_helpers;
pub mod kernel_memory_candidates;
pub mod kernel_memory_feedback;
pub mod kernel_metrics;
pub mod kernel_ops;
pub mod kernel_prompts;
pub mod kernel_proposals;
pub mod kernel_review;
pub mod kernel_run_loop;
pub mod kernel_task_packet;

// ── Observation & Context ──
pub mod context;
pub mod context_relevance;
pub mod observation;
pub mod story_impact;

// ── Proposals & Operations ──
pub mod operation;
pub mod proposal;
pub mod run_events;
pub mod trajectory;

// ── Authoring Modules ──
pub mod author_voice;
pub mod belief_conflict;
pub mod canon;
pub mod chapter_settlement;
pub mod diagnostics;
pub mod inspector;
pub mod post_write_diagnostics;
pub mod project_intake;
pub mod promise_planner;
pub mod research_subtask;
pub mod rewrite_impact;
pub mod supervised_sprint;

// ── Memory & Feedback ──
pub mod feedback;
pub mod memory;
pub mod metacognition;

// ── Planner & Budget ──
pub mod intent;
pub mod provider_budget;
pub mod run_preflight;
pub mod task_receipt;

// ── Re-exports ──
pub use feedback::ProposalFeedback;
pub use kernel::{WriterAgentKernel, WriterAgentStatus};
