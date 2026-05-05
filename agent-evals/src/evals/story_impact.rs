#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::context::AgentTask;
use agent_writer_lib::writer_agent::kernel::{
    WriterAgentApprovalMode, WriterAgentFrontendState, WriterAgentRunRequest,
    WriterAgentStreamMode, WriterAgentTask,
};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::observation::{ObservationReason, ObservationSource};
use agent_writer_lib::writer_agent::story_impact::{
    build_story_graph, compute_story_impact, compute_story_impact_radius, extract_seed_nodes,
    StoryEdgeKind, StoryImpactRisk, StoryNodeKind, WriterStoryGraphEdge, WriterStoryGraphNode,
};
use agent_writer_lib::writer_agent::WriterAgentKernel;

include!("story_impact/part_a.in.rs");
include!("story_impact/part_b.in.rs");
