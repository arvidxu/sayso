use dictation_types::{AppResult, AsrCaps, AsrOptions, ProviderId, Transcript};

/// One recognition session, created by [`AsrEngine::start`]. It takes at most
/// `AsrCaps::max_duration_secs` of audio; the caller splits longer recordings across sessions.
pub trait AsrSession: Send {
    /// Feeds mono PCM samples (`f32` between -1 and 1) at the sample rate the engine declares
    /// in its capabilities. Returns an updated partial transcript when one is available.
    fn push_audio(&mut self, samples: &[f32]) -> AppResult<Option<Transcript>>;

    /// The most recent partial transcript, without ending the session.
    fn partial_transcript(&self) -> AppResult<Option<Transcript>> {
        Ok(None)
    }

    fn finish(&mut self) -> AppResult<Transcript>;

    fn cancel(&mut self) -> AppResult<()> {
        Ok(())
    }
}

pub trait AsrEngine: Send + Sync {
    fn id(&self) -> ProviderId;

    fn capabilities(&self) -> AsrCaps;

    /// Stops what the engine is doing right now, a recognition or getting ready for one;
    /// what was stopped fails as cancelled. An engine that cannot stop in the middle does
    /// nothing, and the work runs to its end.
    fn interrupt(&self) {}

    fn start(&self, opts: AsrOptions) -> AppResult<Box<dyn AsrSession>>;

    /// Transcribes a recording in one call, in one session: the recording has to be no
    /// longer than the engine takes at once. A session that fails part-way is cancelled.
    fn transcribe(&self, samples: &[f32], opts: AsrOptions) -> AppResult<Transcript> {
        let mut session = self.start(opts)?;
        if let Err(err) = session.push_audio(samples) {
            let _ = session.cancel();
            return Err(err);
        }
        session.finish()
    }
}
