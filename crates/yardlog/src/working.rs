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

    /// The agent's own working stage: as `working_stage`, without its session's main agent's
    /// active stage, so that only a task assigned to it or a stage it opened gives it one.
    #[must_use]
    pub fn own_stage(&self, agent: &Agent, task: Option<&str>) -> Option<StageKey> {
        match self.mine(agent, task, true) {
            StageContext::Stage(stage) => Some(stage),
            StageContext::Ambiguous | StageContext::None => None,
        }
    }

    fn working(&self, agent: &Agent, task: Option<&str>, own: bool) -> Option<StageKey> {
        match self.mine(agent, task, own) {
            StageContext::Stage(stage) => return Some(stage),
            StageContext::Ambiguous => return None,
            StageContext::None => {}
        }
        let main = Agent::main_in(&agent.host, &agent.session);
        self.active_stages_of(&main)
            .next()
            .map(|stage| stage.key.stage())
    }

    /// What the agent's own tasks and stages say of its stage: a named or assigned task's stage,
    /// or, with `own`, the stage it owns; `Ambiguous` where they leave it none, as a named task not
    /// assigned to it does; `None` where they say nothing, and its main agent's stage decides.
    fn mine(&self, agent: &Agent, task: Option<&str>, own: bool) -> StageContext {
        if let Some(id) = task {
            return self
                .assigned_to(agent)
                .find(|(_, block, _)| block.id.as_str() == id)
                .map_or(StageContext::Ambiguous, |(file, _, _)| {
                    StageContext::Stage(file.stage.key.stage())
                });
        }
        match self.delegation_context(agent) {
            StageContext::None => {}
            settled => return settled,
        }
        match self.active_stages_of(agent).next() {
            Some(stage) if own => StageContext::Stage(stage.key.stage()),
            _ => StageContext::None,
        }
    }
}
