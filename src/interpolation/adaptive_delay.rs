/// Adaptive jitter buffer delay calculator.
///
/// Separated from the buffer storage (HistoryBuffer) so it can
/// be tested and tuned independently.
pub struct AdaptiveDelay {
    /// EMA of intervals between snapshots
    avg_interval: f64,
    /// EMA of jitter (deviation from avg)
    jitter: f64,
    /// Time of last snapshot push
    last_push_time: Option<f64>,
    /// EMA smoothing factor
    ema_alpha: f64,
    /// Minimum delay (seconds)
    min_delay: f64,
    /// Maximum delay (seconds)
    max_delay: f64,
    /// Expected server tick interval (for clamping avg_interval)
    expected_interval: f64,
}

impl AdaptiveDelay {
    /// Create a new adaptive delay calculator.
    ///
    /// `server_tps`: expected server tick rate (e.g., 64.0)
    pub fn new(server_tps: f64) -> Self {
        let expected = 1.0 / server_tps;
        Self {
            avg_interval: expected,
            jitter: 0.0,
            last_push_time: None,
            ema_alpha: 0.2,
            min_delay: expected * 6.0,
            max_delay: 0.5,
            expected_interval: expected,
        }
    }

    /// Record a snapshot arrival and update statistics.
    /// `timestamp`: local-time timestamp of the snapshot
    pub fn record_arrival(&mut self, timestamp: f64) {
        if let Some(last) = self.last_push_time {
            let interval = timestamp - last;
            // Ignore anomalous intervals:
            // - Near-zero from TCP bursts
            // - Very large from idle periods
            if interval > 0.001 && interval < self.expected_interval * 8.0 {
                let deviation = (interval - self.avg_interval).abs();
                self.avg_interval =
                    self.avg_interval * (1.0 - self.ema_alpha) + interval * self.ema_alpha;
                self.jitter =
                    self.jitter * (1.0 - self.ema_alpha) + deviation * self.ema_alpha;

                // Clamp avg_interval so TCP bursts don't pull it too low
                self.avg_interval = self.avg_interval.max(self.expected_interval * 0.5);
            }
        }
        self.last_push_time = Some(timestamp);
    }

    /// Get the current adaptive delay.
    /// Formula: 1.5 * avg_interval + 2 * jitter
    pub fn delay(&self) -> f64 {
        let delay = self.avg_interval * 1.5 + self.jitter * 2.0;
        delay.clamp(self.min_delay, self.max_delay)
    }

    pub fn avg_interval(&self) -> f64 {
        self.avg_interval
    }

    pub fn jitter(&self) -> f64 {
        self.jitter
    }
}
