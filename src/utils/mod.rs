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

pub fn floats_to_ints(floats: &[f64], target_sum: i32) -> Vec<i32> {
    let floored: Vec<_> = floats.iter().map(|&x| x.floor() as i32).collect();
    let remainders: Vec<f64> = floats
        .iter()
        .zip(floored.iter())
        .map(|(&original, &floor)| original - floor as f64)
        .collect();

    let current_sum: i32 = floored.iter().sum();
    let mut deficit = target_sum - current_sum;

    let mut indices: Vec<usize> = (0..floats.len()).collect();
    indices.sort_by(|&a, &b| {
        remainders[b]
            .partial_cmp(&remainders[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result = floored;

    if deficit > 0 {
        for &idx in &indices {
            if deficit <= 0 {
                break;
            }
            result[idx] += 1;
            deficit -= 1;
        }
    }

    if deficit < 0 {
        indices.reverse();
        for &idx in &indices {
            if deficit >= 0 {
                break;
            }
            result[idx] -= 1;
            deficit += 1;
        }
    }

    result
}
