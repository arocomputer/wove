//! Time-based interpolation without an executor or implicit render loop.
use std::time::Duration;

/// A finite linear transition driven by the application's monotonic clock.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub start: Duration,
    pub duration: Duration,
}
impl Tween {
    pub fn value(&self, now: Duration) -> f32 {
        if now < self.start {
            return self.from;
        }
        if self.duration.is_zero() {
            return self.to;
        }
        let progress = (now.saturating_sub(self.start).as_secs_f64() / self.duration.as_secs_f64())
            .min(1.0) as f32;
        self.from + (self.to - self.from) * progress
    }
    pub fn finished(&self, now: Duration) -> bool {
        now >= self.start.saturating_add(self.duration)
    }
}
