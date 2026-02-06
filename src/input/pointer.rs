use std::collections::HashMap;

use evdev::KeyCode;
use smithay::{
    backend::input::{ButtonState, InputBackend, PointerButtonEvent, PointerMotionAbsoluteEvent},
    input::pointer::{ButtonEvent, Focus, GrabStartData as PointerGrabStartData, MotionEvent},
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Rectangle, SERIAL_COUNTER},
};

use crate::{
    input::{
        KeyModifiers,
        move_grab::MoveGrab,
        resize_grab::{ResizeEdge, ResizeGrab},
    },
    state::WaylandState,
};

pub type PointerBindings = HashMap<PointerBinding, PointerActions>;

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct PointerBinding {
    modifiers: KeyModifiers,
    code: KeyCode,
}

#[derive(Debug)]
pub enum PointerActions {
    Move,
    Resize,
}

pub enum ResizeLocation {
    Corner,
    Edge,
}

// TEMP
pub fn test_pointer_bindings() -> PointerBindings {
    HashMap::from_iter([
        (
            PointerBinding {
                modifiers: KeyModifiers::ALT,
                code: KeyCode(0x110),
            },
            PointerActions::Move,
        ),
        (
            PointerBinding {
                modifiers: KeyModifiers::ALT,
                code: KeyCode(0x111),
            },
            PointerActions::Resize,
        ),
    ])
}

impl WaylandState {
    pub fn process_pointer_motion_absolute<B: InputBackend, T: PointerMotionAbsoluteEvent<B>>(
        &mut self,
        event: T,
    ) {
        let output = self.space.outputs().next().unwrap();
        let output_geo = self.space.output_geometry(output).unwrap();

        let location = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();

        let serial = SERIAL_COUNTER.next_serial();

        let pointer = self.seat.get_pointer().unwrap();

        let under = self.surface_under(location);

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
            && let Some((mapped, _)) = self.mapped_window_under(pointer.current_location())
        {
            let surface = mapped.toplevel().wl_surface().clone();
            self.focus_window(&surface, Some(serial));
        }

        if button_state == ButtonState::Pressed
            && let Some(action) = self.pointer_config.bindings.get(&PointerBinding {
                modifiers: self.key_modifiers,
                code: KeyCode(button as u16),
            })
        {
            use PointerActions::*;
            match action {
                Move => {
                    if let Some((mapped, _)) = self.mapped_window_under(pointer.current_location())
                        && !pointer.is_grabbed()
                    {
                        let location = pointer.current_location();
                        let start_data = PointerGrabStartData {
                            focus: None,
                            button,
                            location,
                        };
                        let grab = MoveGrab::new(
                            start_data,
                            mapped.window.clone(),
                            mapped.location.to_f64(),
                        );
                        pointer.set_grab(self, grab, serial, Focus::Clear);
                    }
                }
                Resize => {
                    if let Some((mapped, _)) = self.mapped_window_under(pointer.current_location())
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
                                let size = mapped.window.geometry().size;
                                let width_1_3 = size.w as f64 * 1. / 3.;
                                let width_2_3 = size.w as f64 * 2. / 3.;
                                let height_1_3 = size.h as f64 * 1. / 3.;
                                let height_2_3 = size.h as f64 * 2. / 3.;

                                let window_location = mapped.location.to_f64();
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
                                    // #[allow(clippy::collapsible_else_if)]
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

                        let grab = ResizeGrab::new(
                            start_data,
                            mapped.window.clone(),
                            edge,
                            Rectangle::new(mapped.location, mapped.window.geometry().size),
                        );
                        pointer.set_grab(self, grab, serial, Focus::Clear);
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
