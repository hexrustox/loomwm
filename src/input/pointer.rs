use std::collections::HashMap;

use evdev::KeyCode;
use smithay::{
    backend::input::{ButtonState, InputBackend, PointerButtonEvent, PointerMotionAbsoluteEvent},
    desktop::WindowSurfaceType,
    input::pointer::{ButtonEvent, Focus, GrabStartData as PointerGrabStartData, MotionEvent},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, SERIAL_COUNTER},
};

use crate::{
    input::{KeyModifiers, move_grab::MoveGrab},
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
}

pub fn test_pointer_bindings() -> PointerBindings {
    HashMap::from_iter([(
        PointerBinding {
            modifiers: KeyModifiers::ALT,
            code: KeyCode(0x110),
        },
        PointerActions::Move,
    )])
}

impl WaylandState {
    pub fn process_pointer_motion_absolute<B: InputBackend, T: PointerMotionAbsoluteEvent<B>>(
        &mut self,
        event: T,
    ) {
        let output = self.space.outputs().next().unwrap();
        let output_geo = self.space.output_geometry(output).unwrap();

        let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();

        let serial = SERIAL_COUNTER.next_serial();

        let pointer = self.seat.get_pointer().unwrap();

        let under = self.surface_under(pos);

        pointer.motion(
            self,
            under,
            &MotionEvent {
                location: pos,
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
            && let Some((mapped, _)) = self
                .workspaces
                .get_active()
                .mapped_window_under(pointer.current_location())
        {
            let surface = mapped.toplevel().wl_surface().clone();
            self.focus_window(&surface, Some(serial));
        }

        if button_state == ButtonState::Pressed
            && let Some(action) = self.pointer_bindings.get(&PointerBinding {
                modifiers: self.key_modifiers,
                code: KeyCode(button as u16),
            })
        {
            match action {
                PointerActions::Move => {
                    if let Some((mapped, _)) = self
                        .workspaces
                        .get_active()
                        .mapped_window_under(pointer.current_location())
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
                            mapped.inner.clone(),
                            mapped.location.to_f64(),
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

    pub fn surface_under(
        &mut self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.workspaces
            .get_active()
            .mapped_window_under(pos)
            .and_then(|(window, location)| {
                window
                    .inner
                    .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            })
    }
}
