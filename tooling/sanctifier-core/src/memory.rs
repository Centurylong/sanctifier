use std::fmt;

#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    pub current_rss_kb: u64,
    pub peak_rss_kb: u64,
}

pub struct MemoryTracker {
    peak_kb: u64,
}

impl MemoryTracker {
    pub fn new() -> Self {
        Self { peak_kb: 0 }
    }

    pub fn sample(&mut self) -> MemorySnapshot {
        let current = read_rss_kb();
        self.peak_kb = self.peak_kb.max(current);
        MemorySnapshot {
            current_rss_kb: current,
            peak_rss_kb: self.peak_kb,
        }
    }

    pub fn peak_kb(&self) -> u64 {
        self.peak_kb
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct MemoryGuard {
    limit_kb: u64,
}

impl MemoryGuard {
    pub fn new(limit_mb: u64) -> Self {
        Self {
            limit_kb: limit_mb * 1024,
        }
    }

    pub fn check(&self, tracker: &MemoryTracker) -> Result<(), MemoryLimitExceeded> {
        if tracker.peak_kb() > self.limit_kb {
            Err(MemoryLimitExceeded {
                peak_kb: tracker.peak_kb(),
                limit_kb: self.limit_kb,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryLimitExceeded {
    pub peak_kb: u64,
    pub limit_kb: u64,
}

impl fmt::Display for MemoryLimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "memory limit exceeded: peak {} MB > limit {} MB",
            self.peak_kb / 1024,
            self.limit_kb / 1024
        )
    }
}

impl std::error::Error for MemoryLimitExceeded {}

fn read_rss_kb() -> u64 {
    read_rss_kb_proc().unwrap_or(0)
}

fn read_rss_kb_proc() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let value = line.split_whitespace().nth(1)?.parse::<u64>().ok()?;
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_tracker_starts_at_zero_peak() {
        let tracker = MemoryTracker::new();
        assert_eq!(tracker.peak_kb(), 0);
    }

    #[test]
    fn memory_tracker_samples_increase_peak() {
        let mut tracker = MemoryTracker::new();
        let snap = tracker.sample();
        assert!(snap.current_rss_kb > 0, "RSS should be positive on Linux");
        assert!(snap.peak_rss_kb > 0);
        assert_eq!(snap.current_rss_kb, snap.peak_rss_kb);
    }

    #[test]
    fn memory_tracker_peak_is_monotonic() {
        let mut tracker = MemoryTracker::new();
        let first = tracker.sample();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let second = tracker.sample();
        assert!(second.peak_rss_kb >= first.peak_rss_kb);
    }

    #[test]
    fn memory_guard_allows_below_limit() {
        let mut tracker = MemoryTracker::new();
        let _ = tracker.sample();
        let guard = MemoryGuard::new(1024 * 1024); // 1 TB, far above any real RSS
        assert!(guard.check(&tracker).is_ok());
    }

    #[test]
    fn memory_guard_rejects_above_zero_limit() {
        let mut tracker = MemoryTracker::new();
        tracker.sample();
        // Set limit to 0 KB — impossible to be under
        let guard = MemoryGuard { limit_kb: 0 };
        assert!(guard.check(&tracker).is_err());
    }

    #[test]
    fn memory_limit_exceeded_display_shows_mb() {
        let err = MemoryLimitExceeded {
            peak_kb: 2048,
            limit_kb: 1024,
        };
        let msg = err.to_string();
        assert!(msg.contains("2 MB"));
        assert!(msg.contains("1 MB"));
    }
}
