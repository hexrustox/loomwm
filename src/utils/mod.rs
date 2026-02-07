use std::time::Duration;

use smithay::{
    backend::renderer::utils::with_renderer_surface_state,
    reexports::{
        rustix::time::{ClockId, clock_gettime},
        wayland_server::protocol::wl_surface::WlSurface,
    },
    wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData},
};

pub fn get_monotonic_time() -> Duration {
    let ts = clock_gettime(ClockId::Monotonic);
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

pub fn is_mapped(surface: &WlSurface) -> bool {
    with_renderer_surface_state(surface, |state| state.buffer().is_some()).unwrap_or(false)
}

pub fn partition(sum: i32, split_into: usize) -> Vec<i32> {
    if split_into == 0 {
        return Vec::new();
    }

    let split_into = split_into as i32;
    let base = sum / split_into;
    let remainder = sum - base * split_into;

    (0..split_into)
        .map(|i| {
            if remainder >= 0 {
                if i < remainder { base + 1 } else { base }
            } else if i < -remainder {
                base - 1
            } else {
                base
            }
        })
        .collect()
}

pub fn get_app_id_and_title(surface: &WlSurface) -> (String, String) {
    with_states(surface, |surface_data| {
        if let Some(attrs) = surface_data.data_map.get::<XdgToplevelSurfaceData>() {
            let attrs = attrs.lock().unwrap();
            (
                attrs.app_id.clone().unwrap_or_default(),
                attrs.title.clone().unwrap_or_default(),
            )
        } else {
            (String::default(), String::default())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{prelude::*, test_runner::TestRunner};

    #[test]
    fn test_partition() {
        let mut runner = TestRunner::default();
        runner
            .run(
                // i32 takes too long
                &any::<i16>().prop_flat_map(|a| {
                    let b = a.unsigned_abs() as usize;
                    (Just(a), 0..b)
                }),
                |(a, b)| {
                    let parts = partition(a as i32, b);
                    let avg = a as f64 / b as f64;

                    for p in &parts {
                        assert!((*p as f64 - avg).abs() <= 1.0);
                    }
                    assert!(parts.iter().sum::<i32>() == a as i32);

                    Ok(())
                },
            )
            .unwrap();
    }
}
