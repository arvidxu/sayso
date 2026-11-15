#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTiming {
    pub stage: String,
    pub ms: u64,
}

/// In recording order; names are chosen by whoever runs the stage.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StageTimings {
    pub stages: Vec<StageTiming>,
    /// End-to-end time, when measured separately.
    pub total_ms: Option<u64>,
}

impl StageTimings {
    /// Adds `ms` to `stage`, creating it on first use.
    pub fn record(&mut self, stage: impl Into<String>, ms: u64) {
        let stage = stage.into();
        match self.stages.iter_mut().find(|t| t.stage == stage) {
            Some(existing) => existing.ms = existing.ms.saturating_add(ms),
            None => self.stages.push(StageTiming { stage, ms }),
        }
    }

    pub fn get(&self, stage: &str) -> Option<u64> {
        self.stages.iter().find(|t| t.stage == stage).map(|t| t.ms)
    }

    /// The measured total, or the sum of the stages when no total was measured.
    pub fn computed_total_ms(&self) -> u64 {
        self.total_ms.unwrap_or_else(|| {
            self.stages
                .iter()
                .fold(0u64, |sum, t| sum.saturating_add(t.ms))
        })
    }
}
