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

pub fn evenly_div(lhs: i32, rhs: i32) -> Vec<i32> {
    let mut vec = Vec::new();
    let ans = lhs.wrapping_div_euclid(rhs);
    if lhs % rhs == 0 {
        for _ in 0..rhs {
            vec.push(ans);
        }
    } else {
        let remainder = lhs - ans * rhs;
        println!("{ans} {remainder}");
        for i in 0..rhs {
            vec.push(if i < remainder { ans + 1 } else { ans });
        }
    }
    vec
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(3, 3 => vec![1, 1, 1])]
    #[test_case(8, 3 => vec![3, 3, 2])]
    #[test_case(9, 4 => vec![3, 2, 2, 2])]
    fn test_evenly_div(lhs: i32, rhs: i32) -> Vec<i32> {
        evenly_div(lhs, rhs)
    }
}
