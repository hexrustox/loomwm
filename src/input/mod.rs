use smithay::{
    backend::input::{
        AbsolutePositionEvent, ButtonState, Event, InputBackend, InputEvent, KeyboardKeyEvent,
        PointerButtonEvent,
    },
    desktop::WindowSurfaceType,
    input::{
        keyboard::FilterResult,
        pointer::{ButtonEvent, MotionEvent},
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, SERIAL_COUNTER, Serial},
};

use crate::state::WMState;

pub mod move_grab;

impl WMState {
    pub fn process_input_event<T: InputBackend>(&mut self, event: InputEvent<T>) {
        use InputEvent::*;
        match event {
            Keyboard { event } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);

                self.seat.get_keyboard().unwrap().input::<(), _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    time,
                    |_, _, _| FilterResult::Forward,
                );
            }
            PointerMotionAbsolute { event } => {
                let pos = event.position();
                let pos = Point::<f64, Logical>::new(pos.x, pos.y);

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
            PointerButton { event } => {
                let pointer = self.seat.get_pointer().unwrap();

                let serial = SERIAL_COUNTER.next_serial();
                let button = event.button_code();
                let button_state = event.state();

                if button_state == ButtonState::Pressed
                    && let Some((window, _)) = self.windows.window_under(pointer.current_location())
                {
                    let surface = window.toplevel().unwrap().wl_surface().clone();
                    self.focus_window(&surface, Some(serial));
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
            _ => {}
        }
    }

    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.windows
            .window_under(pos)
            .and_then(|(window, location)| {
                window
                    .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            })
    }

    pub fn focus_window(&mut self, surface: &WlSurface, serial: Option<Serial>) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let serial = serial.unwrap_or(SERIAL_COUNTER.next_serial());
        if let Some(surface) = keyboard.current_focus()
            && let Some(mapped) = self.windows.mapped_windows.get(&surface)
        {
            mapped.inner.set_activated(false);
            mapped.toplevel().send_pending_configure();
        }
        keyboard.set_focus(self, Some(surface.clone()), serial);
        if let Some(mapped) = self.windows.mapped_windows.get(surface) {
            mapped.inner.set_activated(true);
            mapped.toplevel().send_pending_configure();
        }
    }
}
