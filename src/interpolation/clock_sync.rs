/// Tracks the offset between local time and server time.
///
/// Uses a median-based approach instead of pure EMA, which is
/// more robust against outliers from TCP burst delivery.
///
/// offset = local_time - server_time
/// local_timestamp = server_timestamp + offset
pub struct ClockSync {
    /// Recent offset samples for median calculation
    samples: Vec<f64>,
    /// Maximum samples to keep
    max_samples: usize,
    /// Current best estimate of offset
    offset: f64,
    /// Whether we have a valid offset
    initialized: bool,
    /// EMA smoothing factor for gradual drift correction
    ema_alpha: f64,
}

impl ClockSync {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples),
            max_samples,
            offset: 0.0,
            initialized: false,
            ema_alpha: 0.1,
        }
    }

    /// Record a new offset sample.
    /// `local_time`: current local monotonic time (seconds)
    /// `server_time`: server timestamp from the packet
    pub fn record_sample(&mut self, local_time: f64, server_time: f64) {
        let sample = local_time - server_time;

        if !self.initialized {
            self.offset = sample;
            self.initialized = true;
            self.samples.push(sample);
            return;
        }

        // Add to rolling window
        if self.samples.len() >= self.max_samples {
            self.samples.remove(0);
        }
        self.samples.push(sample);

        // Use median of recent samples for robustness,
        // then EMA-smooth toward it to avoid jumps
        if self.samples.len() >= 3 {
            let mut sorted = self.samples.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = sorted[sorted.len() / 2];
            self.offset = self.offset * (1.0 - self.ema_alpha) + median * self.ema_alpha;
        } else {
            self.offset = self.offset * (1.0 - self.ema_alpha) + sample * self.ema_alpha;
        }
    }

    /// Convert a server timestamp to local time
    pub fn server_to_local(&self, server_time: f64) -> f64 {
        server_time + self.offset
    }

    pub fn get_offset(&self) -> f64 {
        self.offset
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}
