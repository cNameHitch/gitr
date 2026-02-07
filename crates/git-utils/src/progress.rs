use std::io::{self, Write};
use std::time::Instant;

/// Progress display on stderr, matching C git's progress.c behavior.
///
/// Displays progress updates like:
/// - `Counting objects: 42` (no total)
/// - `Counting objects:  50% (42/84)` (with total)
/// - `Counting objects:  50% (42/84), 1.23 MiB | 456.00 KiB/s` (with throughput)
pub struct Progress {
    title: String,
    total: Option<u64>,
    current: u64,
    start_time: Instant,
    last_update: Instant,
    /// Minimum delay between display updates in milliseconds.
    delay_ms: u64,
    /// Whether the first update has been displayed.
    started: bool,
    /// Whether to delay the first display (like C git's start_delayed_progress).
    initial_delay_ms: u64,
    /// Throughput tracking.
    throughput: Option<ThroughputState>,
    /// Last percentage displayed (to avoid redundant updates).
    last_percent: Option<u32>,
}

/// Internal throughput tracking state.
struct ThroughputState {
    last_bytes: u64,
    last_time: Instant,
    avg_bytes: f64,
    avg_seconds: f64,
}

impl Progress {
    /// Create a new progress display with a title and optional total count.
    pub fn new(title: &str, total: Option<u64>) -> Self {
        let now = Instant::now();
        Self {
            title: title.to_string(),
            total,
            current: 0,
            start_time: now,
            last_update: now,
            delay_ms: 100,
            started: false,
            initial_delay_ms: 0,
            throughput: None,
            last_percent: None,
        }
    }

    /// Create a delayed progress display that waits before showing output.
    ///
    /// Matches C git's `start_delayed_progress()`.
    pub fn delayed(title: &str, total: Option<u64>, initial_delay_ms: u64) -> Self {
        let mut p = Self::new(title, total);
        p.initial_delay_ms = initial_delay_ms;
        p
    }

    /// Enable throughput display.
    pub fn enable_throughput(&mut self) {
        let now = Instant::now();
        self.throughput = Some(ThroughputState {
            last_bytes: 0,
            last_time: now,
            avg_bytes: 0.0,
            avg_seconds: 0.0,
        });
    }

    /// Update the throughput counter with the total bytes processed so far.
    pub fn display_throughput(&mut self, total_bytes: u64) {
        if let Some(ref mut tp) = self.throughput {
            let now = Instant::now();
            let elapsed = now.duration_since(tp.last_time).as_secs_f64();
            if elapsed > 0.0 {
                let bytes_delta = total_bytes.saturating_sub(tp.last_bytes) as f64;
                // Exponential moving average
                tp.avg_bytes = tp.avg_bytes * 0.875 + bytes_delta * 0.125;
                tp.avg_seconds = tp.avg_seconds * 0.875 + elapsed * 0.125;
                tp.last_bytes = total_bytes;
                tp.last_time = now;
            }
        }
    }

    /// Update the progress count.
    pub fn update(&mut self, count: u64) {
        self.current = count;

        let now = Instant::now();
        let since_last = now.duration_since(self.last_update).as_millis() as u64;

        // If we haven't started and there's an initial delay, check it
        if !self.started {
            let since_start = now.duration_since(self.start_time).as_millis() as u64;
            if since_start < self.initial_delay_ms {
                return;
            }
        }

        // Rate-limit updates
        if self.started && since_last < self.delay_ms {
            // Still display at 100% even if rate-limited
            if let Some(total) = self.total {
                if count < total {
                    return;
                }
            } else {
                return;
            }
        }

        // Check if we actually need to update (percentage changed or no total)
        if let Some(total) = self.total {
            if total > 0 {
                let percent = ((count as f64 / total as f64) * 100.0) as u32;
                if self.started && self.last_percent == Some(percent) && count < total {
                    return;
                }
                self.last_percent = Some(percent);
            }
        }

        self.started = true;
        self.last_update = now;
        self.display();
    }

    /// Increment the count by one.
    pub fn tick(&mut self) {
        self.update(self.current + 1);
    }

    /// Display the current progress on stderr.
    fn display(&self) {
        let mut stderr = io::stderr();

        let counters = match self.total {
            Some(total) if total > 0 => {
                let percent = (self.current as f64 / total as f64) * 100.0;
                format!(
                    "\r{}: {:3.0}% ({}/{})",
                    self.title, percent, self.current, total
                )
            }
            _ => {
                format!("\r{}: {}", self.title, self.current)
            }
        };

        let throughput_str = self.format_throughput();

        let line = if throughput_str.is_empty() {
            counters
        } else {
            format!("{}, {}", counters, throughput_str)
        };

        let _ = write!(stderr, "{}", line);
        let _ = stderr.flush();
    }

    /// Format the throughput string.
    fn format_throughput(&self) -> String {
        if let Some(ref tp) = self.throughput {
            if tp.avg_seconds > 0.0 {
                let bytes_per_sec = tp.avg_bytes / tp.avg_seconds;
                let (value, unit) = human_readable_bytes(bytes_per_sec);
                let (total_value, total_unit) = human_readable_bytes(tp.last_bytes as f64);
                format!(
                    "{:.2} {} | {:.2} {}/s",
                    total_value, total_unit, value, unit
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    }

    /// Finish and clear the progress line, printing "done" on stderr.
    pub fn finish(self) {
        let mut stderr = io::stderr();
        if self.started {
            let elapsed = self.start_time.elapsed();
            let elapsed_str = if elapsed.as_secs() > 0 {
                format!(", {:.2}s", elapsed.as_secs_f64())
            } else {
                String::new()
            };

            match self.total {
                Some(total) if total > 0 => {
                    let _ = write!(
                        stderr,
                        "\r{}: 100% ({}/{}){}, done.\n",
                        self.title, total, total, elapsed_str
                    );
                }
                _ => {
                    let _ = write!(
                        stderr,
                        "\r{}: {}{}, done.\n",
                        self.title, self.current, elapsed_str
                    );
                }
            }
            let _ = stderr.flush();
        }
    }

    /// Finish with a custom message.
    pub fn finish_with_msg(self, msg: &str) {
        let mut stderr = io::stderr();
        if self.started {
            match self.total {
                Some(total) if total > 0 => {
                    let _ = write!(
                        stderr,
                        "\r{}: 100% ({}/{}), {}.\n",
                        self.title, total, total, msg
                    );
                }
                _ => {
                    let _ = write!(
                        stderr,
                        "\r{}: {}, {}.\n",
                        self.title, self.current, msg
                    );
                }
            }
            let _ = stderr.flush();
        }
    }
}

/// Convert bytes to human-readable format (matching C git's strbuf_humanise_bytes).
fn human_readable_bytes(bytes: f64) -> (f64, &'static str) {
    if bytes >= 1024.0 * 1024.0 * 1024.0 {
        (bytes / (1024.0 * 1024.0 * 1024.0), "GiB")
    } else if bytes >= 1024.0 * 1024.0 {
        (bytes / (1024.0 * 1024.0), "MiB")
    } else if bytes >= 1024.0 {
        (bytes / 1024.0, "KiB")
    } else {
        (bytes, "bytes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_with_total() {
        let mut p = Progress::new("Counting objects", Some(100));
        // Disable rate limiting for tests
        p.delay_ms = 0;
        p.update(50);
        assert_eq!(p.current, 50);
        assert!(p.started);
    }

    #[test]
    fn progress_without_total() {
        let mut p = Progress::new("Receiving objects", None);
        p.delay_ms = 0;
        p.update(42);
        assert_eq!(p.current, 42);
        assert!(p.started);
    }

    #[test]
    fn progress_tick() {
        let mut p = Progress::new("Processing", Some(10));
        p.delay_ms = 0;
        p.tick();
        assert_eq!(p.current, 1);
        p.tick();
        assert_eq!(p.current, 2);
    }

    #[test]
    fn progress_finish() {
        let mut p = Progress::new("Counting", Some(100));
        p.delay_ms = 0;
        p.update(100);
        // finish consumes self, verify it doesn't panic
        p.finish();
    }

    #[test]
    fn progress_finish_with_msg() {
        let mut p = Progress::new("Counting", Some(50));
        p.delay_ms = 0;
        p.update(50);
        p.finish_with_msg("all done");
    }

    #[test]
    fn human_readable_bytes_units() {
        let (v, u) = human_readable_bytes(500.0);
        assert_eq!(u, "bytes");
        assert!((v - 500.0).abs() < 0.01);

        let (v, u) = human_readable_bytes(2048.0);
        assert_eq!(u, "KiB");
        assert!((v - 2.0).abs() < 0.01);

        let (v, u) = human_readable_bytes(2.0 * 1024.0 * 1024.0);
        assert_eq!(u, "MiB");
        assert!((v - 2.0).abs() < 0.01);

        let (v, u) = human_readable_bytes(3.0 * 1024.0 * 1024.0 * 1024.0);
        assert_eq!(u, "GiB");
        assert!((v - 3.0).abs() < 0.01);
    }

    #[test]
    fn delayed_progress_respects_delay() {
        let mut p = Progress::delayed("Slow task", Some(100), 5000);
        // With a 5-second initial delay, updates should not start immediately
        p.update(50);
        assert!(!p.started);
    }

    #[test]
    fn throughput_enable() {
        let mut p = Progress::new("Transfer", Some(1000));
        p.delay_ms = 0;
        p.enable_throughput();
        assert!(p.throughput.is_some());
        p.display_throughput(500);
        p.update(500);
    }
}
