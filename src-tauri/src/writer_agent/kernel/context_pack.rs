use super::*;

impl WriterAgentKernel {
    pub fn ghost_context_pack(&self, observation: &WriterObservation) -> WritingContextPack {
        assemble_observation_context_with_default_budget(
            AgentTask::GhostWriting,
            observation,
            &self.memory,
        )
    }

    pub fn context_pack_for(
        &self,
        task: AgentTask,
        observation: &WriterObservation,
        total_budget: usize,
    ) -> WritingContextPack {
        assemble_observation_context(task, observation, &self.memory, total_budget)
    }

    pub fn context_pack_for_default(
        &self,
        task: AgentTask,
        observation: &WriterObservation,
    ) -> WritingContextPack {
        assemble_observation_context_with_default_budget(task, observation, &self.memory)
    }
}
