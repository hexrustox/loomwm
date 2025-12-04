use std::{
    ops::{Div, Rem},
    time::Duration,
};

use smithay::{
    backend::renderer::utils::with_renderer_surface_state,
    reexports::{
        rustix::time::{ClockId, clock_gettime},
        wayland_server::protocol::wl_surface::WlSurface,
    },
};

pub fn get_monotonic_time() -> Duration {
    let ts = clock_gettime(ClockId::Monotonic);
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

pub fn is_mapped(surface: &WlSurface) -> bool {
    with_renderer_surface_state(surface, |state| state.buffer().is_some()).unwrap_or(false)
}

pub fn distribute_evenly(total: i32, parts: i32) -> Vec<i32> {
    let mut result = Vec::new();
    if parts == 0 {
        return result;
    }

    let base = total.wrapping_div_euclid(parts);
    if total % parts == 0 {
        for _ in 0..parts {
            result.push(base);
        }
    } else {
        let remainder = total - base * parts;
        for i in 0..parts {
            result.push(if i < remainder { base + 1 } else { base });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(3, 3 => vec![1, 1, 1])]
    #[test_case(8, 3 => vec![3, 3, 2])]
    #[test_case(9, 4 => vec![3, 2, 2, 2])]
    fn test_distribute_evenly(lhs: i32, rhs: i32) -> Vec<i32> {
        distribute_evenly(lhs, rhs)
    }
}
