use bitflags::bitflags;
use std::time::Duration;

use smithay::{
    backend::renderer::utils::with_renderer_surface_state,
    reexports::{
        rustix::time::{ClockId, clock_gettime},
        wayland_server::protocol::wl_surface::WlSurface,
    },
    wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData},
};

pub mod types;

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

bitflags! {
    #[derive(Debug, Default, Clone, Copy, PartialEq)]
    pub struct Direction: u32 {
        const TOP          = 0b0001;
        const BOTTOM       = 0b0010;
        const LEFT         = 0b0100;
        const RIGHT        = 0b1000;

        const TOP_LEFT     = Self::TOP.bits() | Self::LEFT.bits();
        const BOTTOM_LEFT  = Self::BOTTOM.bits() | Self::LEFT.bits();

        const TOP_RIGHT    = Self::TOP.bits() | Self::RIGHT.bits();
        const BOTTOM_RIGHT = Self::BOTTOM.bits() | Self::RIGHT.bits();
    }
}

impl Direction {
    pub fn opposite(mut self) -> Self {
        if self.contains(Direction::TOP) {
            self.remove(Direction::TOP);
            self.insert(Direction::BOTTOM);
        } else if self.contains(Direction::BOTTOM) {
            self.remove(Direction::BOTTOM);
            self.insert(Direction::TOP);
        }
        if self.contains(Direction::LEFT) {
            self.remove(Direction::LEFT);
            self.insert(Direction::RIGHT);
        } else if self.contains(Direction::RIGHT) {
            self.remove(Direction::RIGHT);
            self.insert(Direction::LEFT);
        }

        self
    }
}

#[derive(Debug, Default, Clone, Hash, PartialEq)]
pub struct RGBAColor(u32);

impl RGBAColor {
    pub fn new(v: u32) -> Self {
        Self(v)
    }

    pub fn r(&self) -> u8 {
        (self.0 >> 24) as u8
    }

    pub fn g(&self) -> u8 {
        (self.0 >> 16) as u8
    }

    pub fn b(&self) -> u8 {
        (self.0 >> 8) as u8
    }

    pub fn a(&self) -> u8 {
        (self.0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(0xFF804020, 0xFF, 0x80, 0x40, 0x20; "opaque")]
    #[test_case(0x00000000, 0x00, 0x00, 0x00, 0x00; "fully_transparent")]
    #[test_case(0xFFFFFFFF, 0xFF, 0xFF, 0xFF, 0xFF; "white")]
    fn test_rgba_color(rgba: u32, r: u8, g: u8, b: u8, a: u8) {
        let color = RGBAColor::new(rgba);
        assert_eq!(color.r(), r);
        assert_eq!(color.g(), g);
        assert_eq!(color.b(), b);
        assert_eq!(color.a(), a);
    }
}
