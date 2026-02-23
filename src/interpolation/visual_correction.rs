use super::traits::Diffable;

/// Manages exponential decay of visual error after a correction.
///
/// When the server says "entity is at X" but we rendered it at Y,
/// we don't snap to X. Instead we:
/// 1. Compute error = Y - X (the visual delta)
/// 2. Each frame, multiply error by a decay factor
/// 3. Render at X + decayed_error, so the entity smoothly slides to X
pub struct VisualCorrection<T: Diffable> {
    /// Current error delta being decayed
    error: Option<T::Delta>,
    /// Decay rate per second. Higher = faster correction.
    /// 10.0 means error reaches ~5% in ~0.3 seconds.
    decay_rate: f32,
    /// Errors smaller than this are zeroed out
    threshold: f32,
}

impl<T: Diffable> VisualCorrection<T> {
    pub fn new(decay_rate: f32, threshold: f32) -> Self {
        Self {
            error: None,
            decay_rate,
            threshold,
        }
    }

    /// Record a correction: we rendered `visual_pos` but the true
    /// position should be `server_pos`. The difference becomes
    /// the new error to decay.
    pub fn record_correction(&mut self, visual_pos: &T, server_pos: &T) {
        let new_error = visual_pos.diff(server_pos);
        if T::delta_is_negligible(&new_error, self.threshold) {
            return;
        }
        self.error = Some(new_error);
    }

    /// Decay the error by delta_time seconds.
    pub fn update(&mut self, delta_time: f32) {
        if let Some(ref error) = self.error {
            let factor = (-self.decay_rate * delta_time).exp();
            let decayed = T::scale_delta(error, factor);
            if T::delta_is_negligible(&decayed, self.threshold) {
                self.error = None;
            } else {
                self.error = Some(decayed);
            }
        }
    }

    /// Apply the current error offset to a position.
    /// Returns the visually corrected position.
    pub fn apply(&self, server_pos: &T) -> T {
        match &self.error {
            Some(error) => server_pos.apply_delta(error),
            None => server_pos.clone(),
        }
    }

    pub fn has_active_correction(&self) -> bool {
        self.error.is_some()
    }

    /// Force clear the error (e.g., on teleport)
    pub fn clear(&mut self) {
        self.error = None;
    }
}
