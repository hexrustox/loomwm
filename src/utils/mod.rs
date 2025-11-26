use std::time::Duration;

use smithay::reexports::rustix::time::{ClockId, clock_gettime};

pub fn get_monotonic_time() -> Duration {
    let ts = clock_gettime(ClockId::Monotonic);
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}
