use std::collections::VecDeque;

/// A timestamped snapshot in the history buffer.
#[derive(Clone)]
pub struct TimestampedSnapshot<T: Clone> {
    pub value: T,
    pub timestamp: f64,
}

/// Fixed-capacity circular buffer of timestamped snapshots.
/// Entries are ordered by timestamp. Binary search for lookups.
pub struct HistoryBuffer<T: Clone> {
    buffer: VecDeque<TimestampedSnapshot<T>>,
    capacity: usize,
}

impl<T: Clone> HistoryBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a new snapshot. Drops oldest if at capacity.
    /// Ignores out-of-order snapshots (timestamp must be >= last).
    pub fn push(&mut self, value: T, timestamp: f64) {
        if let Some(last) = self.buffer.back() {
            if timestamp < last.timestamp {
                return;
            }
        }
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(TimestampedSnapshot { value, timestamp });
    }

    /// Find the pair of snapshots surrounding `time`, plus interpolation factor.
    pub fn sample(&self, time: f64) -> SampleResult<T> {
        if self.buffer.is_empty() {
            return SampleResult::Empty;
        }
        if self.buffer.len() == 1 {
            return SampleResult::Single(self.buffer[0].clone());
        }

        // Before all data
        if time <= self.buffer[0].timestamp {
            return SampleResult::Single(self.buffer[0].clone());
        }

        // After all data -- extrapolation territory
        let last_idx = self.buffer.len() - 1;
        if time > self.buffer[last_idx].timestamp {
            let overtime = time - self.buffer[last_idx].timestamp;
            return SampleResult::Extrapolate {
                prev: self.buffer[last_idx - 1].clone(),
                last: self.buffer[last_idx].clone(),
                overtime,
            };
        }

        // Binary search for the pair bracketing `time`
        let idx = self.buffer.partition_point(|s| s.timestamp <= time);
        let before = &self.buffer[idx - 1];
        let after = &self.buffer[idx];
        let total = after.timestamp - before.timestamp;
        let t = if total > 0.0 {
            ((time - before.timestamp) / total) as f32
        } else {
            0.0
        };
        SampleResult::Interpolate {
            before: before.clone(),
            after: after.clone(),
            t,
        }
    }

    /// Remove snapshots that will never be needed again.
    /// Keeps at least `min_keep` entries.
    pub fn cleanup_before(&mut self, time: f64, min_keep: usize) {
        while self.buffer.len() > min_keep {
            if self.buffer.len() > 1 && self.buffer[1].timestamp < time {
                self.buffer.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn latest(&self) -> Option<&TimestampedSnapshot<T>> {
        self.buffer.back()
    }
}

pub enum SampleResult<T: Clone> {
    Empty,
    Single(TimestampedSnapshot<T>),
    /// Normal interpolation between two snapshots
    Interpolate {
        before: TimestampedSnapshot<T>,
        after: TimestampedSnapshot<T>,
        /// Interpolation factor 0.0..1.0
        t: f32,
    },
    /// render_time is past all snapshots
    Extrapolate {
        prev: TimestampedSnapshot<T>,
        last: TimestampedSnapshot<T>,
        /// Seconds past the last snapshot
        overtime: f64,
    },
}
