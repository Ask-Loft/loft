//! The one stage an agent works in, which every part of Yard Client that asks where an agent's
//! work counts takes from here.

use crate::delegation::StageContext;
use crate::{Agent, StageKey, YardLog};

impl YardLog {
    /// The agent's working stage: the stage of the task `task` names among those assigned to
    /// it, else the one stage its active assigned tasks lie in, else its own active stage, else
    /// its session's main agent's active stage. A named task that is not assigned to the agent,
    /// or active assigned tasks in more than one stage, leave it none.
    #[must_use]
    pub fn working_stage(&self, agent: &Agent, task: Option<&str>) -> Option<StageKey> {
        self.working(agent, task, true)
    }

    /// The stage a subagent's run starts in: as `working_stage`, without the stages it owns, so
    /// that a run with no assigned task works for its session's main agent's stage as it starts.
    #[must_use]
    pub fn starting_stage(&self, agent: &Agent, task: Option<&str>) -> Option<StageKey> {
        self.working(agent, task, false)
    }

    fn working(&self, agent: &Agent, task: Option<&str>, own: bool) -> Option<StageKey> {
        if let Some(id) = task {
            return self
                .assigned_to(agent)
                .find(|(_, block, _)| block.id.as_str() == id)
                .map(|(file, _, _)| file.stage.key.stage());
        }
        match self.delegation_context(agent) {
            StageContext::Stage(stage) => return Some(stage),
            StageContext::Ambiguous => return None,
            StageContext::None => {}
        }
        if own && let Some(stage) = self.active_stages_of(agent).next() {
            return Some(stage.key.stage());
        }
        let main = Agent::main_in(&agent.host, &agent.session);
        self.active_stages_of(&main)
            .next()
            .map(|stage| stage.key.stage())
    }
}
