use crate::rc4::RC4;
use crate::runtime::{EntityIdx, NormalizedUpdateFrame, RuntimeFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RngCheckpoint {
    pub i: u32,
    pub j: u32,
    pub byte_count: u64,
}

impl RngCheckpoint {
    pub fn from_rc4(rng: &RC4) -> Self {
        Self {
            i: rng.i,
            j: rng.j,
            byte_count: rng.byte_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceAction {
    pub round: u64,
    pub actor: EntityIdx,
    pub target: EntityIdx,
    pub amount: i32,
    pub rng_before: Option<RngCheckpoint>,
    pub rng_after: Option<RngCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceFrame {
    pub round: u64,
    pub updates: Vec<NormalizedUpdateFrame>,
    pub total_score: u64,
    pub winner_team: Option<usize>,
    pub rng_after: Option<RngCheckpoint>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeTrace {
    pub actions: Vec<TraceAction>,
    pub frames: Vec<TraceFrame>,
}

impl RuntimeTrace {
    pub fn record_action(&mut self, action: TraceAction) { self.actions.push(action); }

    pub fn record_frame(
        &mut self,
        round: u64,
        frame: &RuntimeFrame,
        winner_team: Option<usize>,
        rng_after: Option<RngCheckpoint>,
    ) {
        let outcome = crate::runtime::RoundOutcome {
            action: None,
            frame: Some(frame.clone()),
            winner_team,
        };
        let updates = NormalizedUpdateFrame::from_outcome(&outcome);
        let total_score = updates.iter().map(|update| u64::from(update.score)).sum();
        self.frames.push(TraceFrame {
            round,
            updates,
            total_score,
            winner_team,
            rng_after,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::RuntimeFrame;

    #[test]
    fn runtime_trace_records_action_and_frame_boundaries() {
        let mut trace = RuntimeTrace::default();
        trace.record_action(TraceAction {
            round: 1,
            actor: EntityIdx(0),
            target: EntityIdx(1),
            amount: 3,
            rng_before: Some(RngCheckpoint {
                i: 1,
                j: 2,
                byte_count: 3,
            }),
            rng_after: Some(RngCheckpoint {
                i: 4,
                j: 5,
                byte_count: 6,
            }),
        });
        trace.record_frame(
            1,
            &RuntimeFrame::single_damage(0, 1, 3),
            None,
            Some(RngCheckpoint {
                i: 7,
                j: 8,
                byte_count: 9,
            }),
        );

        assert_eq!(trace.actions.len(), 1);
        assert_eq!(trace.actions[0].actor, EntityIdx(0));
        assert_eq!(trace.frames.len(), 1);
        assert_eq!(trace.frames[0].updates[0].message, "[0]攻击[1]");
        assert_eq!(trace.frames[0].total_score, 3);
        assert_eq!(
            trace.frames[0].rng_after,
            Some(RngCheckpoint {
                i: 7,
                j: 8,
                byte_count: 9,
            })
        );
    }
}
