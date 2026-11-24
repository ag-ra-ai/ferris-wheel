//! Fixed-timestep bookkeeping: turns real elapsed time into a whole number of
//! equal-length game updates.

use std::time::Duration;

/// Longest real time a single frame may count for. Without this cap, one slow
/// frame (a breakpoint, a dragged window) would trigger a burst of catch-up
/// updates that makes the next frame slow too.
const MAX_FRAME_TIME: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub(crate) struct FixedTimestep {
    step: Duration,
    accumulator: Duration,
}

impl FixedTimestep {
    pub(crate) fn new(updates_per_second: u32) -> Self {
        Self {
            step: Duration::from_secs(1) / updates_per_second.max(1),
            accumulator: Duration::ZERO,
        }
    }

    /// Length of one update.
    pub(crate) fn step(&self) -> Duration {
        self.step
    }

    /// Adds real elapsed time and returns how many updates to run now.
    /// Leftover time carries over to the next call.
    pub(crate) fn advance(&mut self, elapsed: Duration) -> u32 {
        self.accumulator += elapsed.min(MAX_FRAME_TIME);
        let mut steps = 0;
        while self.accumulator >= self.step {
            self.accumulator -= self.step;
            steps += 1;
        }
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn step_matches_rate() {
        assert_eq!(FixedTimestep::new(50).step(), ms(20));
    }

    #[test]
    fn short_frames_accumulate() {
        let mut t = FixedTimestep::new(50); // 20 ms per update
        assert_eq!(t.advance(ms(15)), 0);
        assert_eq!(t.advance(ms(15)), 1); // 30 ms total, 10 ms left over
        assert_eq!(t.advance(ms(10)), 1);
    }

    #[test]
    fn long_frames_run_several_updates() {
        let mut t = FixedTimestep::new(50);
        assert_eq!(t.advance(ms(65)), 3);
        assert_eq!(t.advance(ms(15)), 1); // 5 ms carried over + 15 ms
    }

    #[test]
    fn huge_frames_are_capped() {
        let mut t = FixedTimestep::new(50);
        assert_eq!(t.advance(Duration::from_secs(10)), 12); // 250 ms / 20 ms
    }

    #[test]
    fn zero_rate_does_not_panic() {
        assert_eq!(FixedTimestep::new(0).step(), Duration::from_secs(1));
    }
}
