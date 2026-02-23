/// Trait for types that can be linearly interpolated.
pub trait Interpolatable: Clone {
    fn lerp(&self, other: &Self, t: f32) -> Self;
}

/// Trait for types that can compute and apply deltas (diffs).
/// Used by visual correction to store and decay error offsets.
pub trait Diffable: Clone {
    type Delta: Clone;

    /// Compute the difference: `self - other`
    fn diff(&self, other: &Self) -> Self::Delta;

    /// Apply a delta additively: `self + delta`
    fn apply_delta(&self, delta: &Self::Delta) -> Self;

    /// Scale a delta by a factor (for exponential decay)
    fn scale_delta(delta: &Self::Delta, factor: f32) -> Self::Delta;

    /// Check if a delta is negligible (below threshold)
    fn delta_is_negligible(delta: &Self::Delta, threshold: f32) -> bool;
}
