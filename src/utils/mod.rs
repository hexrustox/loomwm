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
