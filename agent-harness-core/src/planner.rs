use serde::{Deserialize, Serialize};

/// A single step in a plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub step: u32,
    pub action: String,
    pub description: String,
    pub query: Option<String>,
    pub focus: Option<String>,
    pub style: Option<String>,
}

/// Complete execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub steps: Vec<PlanStep>,
    pub goal: String,
}

impl ExecutionPlan {
    /// Parse LLM JSON output, with retry-friendly error messages
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str::<ExecutionPlan>(json).map_err(|e| format!("Plan parse error: {}", e))
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }
}

/// State machine for plan execution
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PlanState {
    Idle,
    Planning,
    Executing {
        current: u32,
        total: u32,
        description: String,
    },
    Completed,
    Failed {
        error: String,
    },
}
