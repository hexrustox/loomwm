use std::str::FromStr;
use std::{collections::HashMap, fmt};

use evdev::KeyCode;
use serde::{Deserialize, Deserializer, de};
use smithay::{
    backend::input::{ButtonState, InputBackend, PointerButtonEvent, PointerMotionAbsoluteEvent},
    input::pointer::{ButtonEvent, Focus, GrabStartData as PointerGrabStartData, MotionEvent},
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Rectangle, SERIAL_COUNTER},
};

use crate::input::swap_grab::SwapGrab;
use crate::input::tiling_resize_grab::TilingResizeGrab;
use crate::monitor::{FoundMappedWindow, TileTreeWindow};
use crate::{
    input::{
        KeyModifiers,
        floating_resize_grab::{FloatingResizeGrab, ResizeEdge},
        move_grab::MoveGrab,
    },
    state::WindowManagerState,
};

pub type PointerBindings = HashMap<PointerCombo, PointerActions>;

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct PointerCombo {
    modifiers: KeyModifiers,
    code: KeyCode,
}

impl<'de> Deserialize<'de> for PointerCombo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PointerBindingVisitor;

        impl<'de> de::Visitor<'de> for PointerBindingVisitor {
            type Value = PointerCombo;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a pointer binding string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let parts: Vec<&str> = v.split('+').map(|s| s.trim()).collect();

                if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
                    return Err(E::custom("empty pointer binding"));
                }

                let mut modifiers = KeyModifiers::empty();

                let (key_part, mod_parts) = parts.split_last().unwrap();

                for &m in mod_parts {
                    match m.to_lowercase().as_str() {
                        "ctrl" => modifiers |= KeyModifiers::CTRL,
                        "shift" => modifiers |= KeyModifiers::SHIFT,
                        "alt" => modifiers |= KeyModifiers::ALT,
                        "super" => modifiers |= KeyModifiers::SUPER,
                        _ => return Err(E::custom(format!("unknown modifier: {}", m))),
                    }
                }

                let code = KeyCode::from_str(&key_part.to_uppercase())
                    .map_err(|_| E::custom(format!("unknown key code: {}", key_part)))?;

                Ok(PointerCombo { modifiers, code })
            }
        }

        deserializer.deserialize_str(PointerBindingVisitor)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PointerActions {
    Move,
    Resize,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResizeLocation {
    #[default]
    Corner,
    Edge,
}

impl WindowManagerState {
    pub fn process_pointer_motion_absolute<B: InputBackend, T: PointerMotionAbsoluteEvent<B>>(
        &mut self,
        event: T,
    ) {
        let output = self.space.outputs().next().unwrap();
        let output_geo = self.space.output_geometry(output).unwrap();

        let location = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();

        let serial = SERIAL_COUNTER.next_serial();

        let pointer = self.seat.get_pointer().unwrap();

        let under = self.find_surface_under(location);

        pointer.motion(
            self,
            under,
            &MotionEvent {
                location,
                serial,
                time: event.time_msec(),
            },
        );
        pointer.frame(self);
    }

    pub fn process_pointer_button<B: InputBackend, T: PointerButtonEvent<B>>(&mut self, event: T) {
        let pointer = self.seat.get_pointer().unwrap();

        let serial = SERIAL_COUNTER.next_serial();
        let button = event.button_code();
        let button_state = event.state();

        if button_state == ButtonState::Pressed
            && let Some(FoundMappedWindow { mapped, .. }) =
                self.find_mapped_window_under(pointer.current_location())
        {
            self.focus_window(&mapped.wl_surface());
        }

        if button_state == ButtonState::Pressed
            && let Some(action) = self.pointer_config.bindings.get(&PointerCombo {
                modifiers: self.key_modifiers,
                code: KeyCode(button as u16),
            })
        {
            use PointerActions::*;
            match action {
                Move => {
                    if let Some(FoundMappedWindow { mapped, .. }) =
                        self.find_mapped_window_under(pointer.current_location())
                        && !pointer.is_grabbed()
                    {
                        {
                            let location = pointer.current_location();
                            let start_data = PointerGrabStartData {
                                focus: None,
                                button,
                                location,
                            };
                            if mapped.get_floating() {
                                let grab = MoveGrab::new(
                                    start_data,
                                    mapped.window().clone(),
                                    mapped.get_location().to_f64(),
                                );
                                pointer.set_grab(self, grab, serial, Focus::Clear);
                            } else {
                                let grab = SwapGrab::new(start_data, mapped.window().clone());
                                pointer.set_grab(self, grab, serial, Focus::Clear);
                            }
                        }
                    }
                }
                Resize => {
                    if let Some(FoundMappedWindow { mapped, .. }) =
                        self.find_mapped_window_under(pointer.current_location())
                        && !pointer.is_grabbed()
                    {
                        let location = pointer.current_location();
                        let start_data = PointerGrabStartData {
                            focus: None,
                            button,
                            location,
                        };

                        let toplevel = mapped.toplevel();
                        toplevel.with_pending_state(|state| {
                            state.states.set(xdg_toplevel::State::Resizing);
                        });
                        toplevel.send_pending_configure();
                        let edge = match self.pointer_config.resize {
                            ResizeLocation::Corner => {
                                let center = mapped.center_location().to_f64();
                                if location.x <= center.x && location.y <= center.y {
                                    ResizeEdge::TOP_LEFT
                                } else if location.x >= center.x && location.y <= center.y {
                                    ResizeEdge::TOP_RIGHT
                                } else if location.x <= center.x && location.y >= center.y {
                                    ResizeEdge::BOTTOM_LEFT
                                } else {
                                    ResizeEdge::BOTTOM_RIGHT
                                }
                            }
                            ResizeLocation::Edge => {
                                let size = mapped.get_size();
                                let width_1_3 = size.w as f64 * 1. / 3.;
                                let width_2_3 = size.w as f64 * 2. / 3.;
                                let height_1_3 = size.h as f64 * 1. / 3.;
                                let height_2_3 = size.h as f64 * 2. / 3.;

                                let window_location = mapped.get_location().to_f64();
                                let x = location.x - window_location.x;
                                let y = location.y - window_location.y;

                                if x <= width_1_3 {
                                    if y <= height_1_3 {
                                        ResizeEdge::TOP_LEFT
                                    } else if y <= height_2_3 {
                                        ResizeEdge::LEFT
                                    } else {
                                        ResizeEdge::BOTTOM_LEFT
                                    }
                                } else if x <= width_2_3 {
                                    if y <= height_1_3 {
                                        ResizeEdge::TOP
                                    } else if y <= height_2_3 {
                                        return;
                                    } else {
                                        ResizeEdge::BOTTOM
                                    }
                                } else {
                                    #[allow(clippy::collapsible_else_if)]
                                    if y <= height_1_3 {
                                        ResizeEdge::TOP_RIGHT
                                    } else if y <= height_2_3 {
                                        ResizeEdge::RIGHT
                                    } else {
                                        ResizeEdge::BOTTOM_RIGHT
                                    }
                                }
                            }
                        };

                        if mapped.get_floating() {
                            let grab = FloatingResizeGrab::new(
                                start_data,
                                mapped.window().clone(),
                                edge,
                                Rectangle::new(mapped.get_location(), mapped.get_size()),
                            );
                            pointer.set_grab(self, grab, serial, Focus::Clear);
                        } else {
                            let grab = TilingResizeGrab::new(start_data, edge, location);
                            pointer.set_grab(self, grab, serial, Focus::Clear);
                        }
                    }
                }
            }
        }

        pointer.button(
            self,
            &ButtonEvent {
                serial,
                time: event.time_msec(),
                button,
                state: button_state,
            },
        );
        pointer.frame(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[derive(Deserialize)]
    struct T {
        x: PointerCombo,
    }

    #[test_case(r#""btn_left""#, KeyCode::BTN_LEFT; "single key")]
    fn test_deserialize(input: &str, code: KeyCode) {
        assert_eq!(
            toml::from_str::<T>(&("x = ".to_string() + input))
                .unwrap()
                .x
                .code,
            code
        );
    }

    #[test_case(r#""foo""#)]
    fn test_deserialize_fail(input: &str) {
        assert!(toml::from_str::<T>(&("x = ".to_string() + input)).is_err())
    }
}
