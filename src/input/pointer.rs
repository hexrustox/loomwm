use std::collections::HashMap;

use evdev::KeyCode;
use serde::Deserialize;
use smithay::{
    backend::input::{ButtonState, InputBackend, PointerButtonEvent, PointerMotionAbsoluteEvent},
    input::pointer::{ButtonEvent, Focus, GrabStartData as PointerGrabStartData, MotionEvent},
    utils::{Rectangle, SERIAL_COUNTER},
};

use crate::input::{
    KeyModifiers,
    grabs::{
        floating_resize_grab::FloatingResizeGrab, move_grab::MoveGrab, swap_grab::SwapGrab,
        tiling_resize_grab::TilingResizeGrab,
    },
};
use crate::monitor::{FoundMappedWindow, TileTreeWindow};
use crate::state::WindowManagerState;
use crate::utils::Direction;

pub type PointerBindings = HashMap<PointerCombo, PointerActions>;

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct PointerCombo {
    modifiers: KeyModifiers,
    key: KeyCode,
}

impl PointerCombo {
    pub fn new(modifiers: KeyModifiers, key: KeyCode) -> Self {
        Self { modifiers, key }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerActions {
    Move,
    Resize,
    // TODO
    // Swap
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizeLocation {
    #[default]
    Corner,
    EdgeOrCorner,
    // TODO
    // Edge
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

        let pointer = self.get_pointer();

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
        let pointer = self.get_pointer();

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
                key: KeyCode(button as u16),
            })
        {
            use PointerActions::*;
            match action {
                Move => {
                    if let Some(FoundMappedWindow {
                        mapped,
                        workspace_name: _,
                        ..
                    }) = self.find_mapped_window_under(pointer.current_location())
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
                                    mapped.clone(),
                                    mapped.get_location().to_f64(),
                                );
                                pointer.set_grab(self, grab, serial, Focus::Clear);
                            } else {
                                let hidden = self.get_focused_workspace_floating_window_hidden();
                                if !hidden {
                                    self.set_focused_workspace_floating_window_hidden(
                                        Some(true),
                                        Some(false),
                                    );
                                }

                                mapped.set_is_swap_source(true);

                                let grab = SwapGrab::new(start_data, mapped.clone(), hidden);
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

                        let direction = match self.pointer_config.resize {
                            ResizeLocation::Corner => {
                                let center = mapped.center_location().to_f64();
                                if location.x <= center.x && location.y <= center.y {
                                    Direction::TOP_LEFT
                                } else if location.x >= center.x && location.y <= center.y {
                                    Direction::TOP_RIGHT
                                } else if location.x <= center.x && location.y >= center.y {
                                    Direction::BOTTOM_LEFT
                                } else {
                                    Direction::BOTTOM_RIGHT
                                }
                            }
                            ResizeLocation::EdgeOrCorner => {
                                let size = mapped.get_geometry_size();
                                let width_1_3 = size.w as f64 * 1. / 3.;
                                let width_2_3 = size.w as f64 * 2. / 3.;
                                let height_1_3 = size.h as f64 * 1. / 3.;
                                let height_2_3 = size.h as f64 * 2. / 3.;

                                let window_location = mapped.get_location().to_f64();
                                let x = location.x - window_location.x;
                                let y = location.y - window_location.y;

                                if x <= width_1_3 {
                                    if y <= height_1_3 {
                                        Direction::TOP_LEFT
                                    } else if y <= height_2_3 {
                                        Direction::LEFT
                                    } else {
                                        Direction::BOTTOM_LEFT
                                    }
                                } else if x <= width_2_3 {
                                    if y <= height_1_3 {
                                        Direction::TOP
                                    } else if y <= height_2_3 {
                                        return;
                                    } else {
                                        Direction::BOTTOM
                                    }
                                } else {
                                    #[allow(clippy::collapsible_else_if)]
                                    if y <= height_1_3 {
                                        Direction::TOP_RIGHT
                                    } else if y <= height_2_3 {
                                        Direction::RIGHT
                                    } else {
                                        Direction::BOTTOM_RIGHT
                                    }
                                }
                            }
                        };

                        if mapped.get_floating() {
                            let grab = FloatingResizeGrab::new(
                                start_data,
                                mapped.clone(),
                                direction,
                                Rectangle::new(mapped.get_location(), mapped.get_geometry_size()),
                            );
                            pointer.set_grab(self, grab, serial, Focus::Clear);
                        } else {
                            let grab =
                                TilingResizeGrab::new(start_data, direction, mapped.get_size());
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
