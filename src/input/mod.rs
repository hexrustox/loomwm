use smithay::{
    backend::input::{InputBackend, InputEvent},
    input::{keyboard::KeyboardHandle, pointer::PointerHandle},
};

use crate::state::WindowManagerState;

pub use keyboard::{KeyBindings, KeyCombo, KeyModifiers, WindowUnit};
pub use pointer::{PointerBindings, PointerCombo, ResizeLocation};

pub mod grabs;
mod keyboard;
mod pointer;

impl WindowManagerState {
    pub fn process_input_event<T: InputBackend>(&mut self, event: InputEvent<T>) {
        use InputEvent::*;
        match event {
            Keyboard { event } => {
                self.process_keyboard_event(event);
            }
            PointerMotionAbsolute { event } => {
                self.process_pointer_motion_absolute(event);
            }
            PointerButton { event } => {
                self.process_pointer_button(event);
            }
            _ => {}
        }
    }

    pub fn get_pointer(&self) -> PointerHandle<Self> {
        self.seat
            .get_pointer()
            .expect("Seat does not have a pointer")
    }

    pub fn get_keyboard(&self) -> KeyboardHandle<Self> {
        self.seat
            .get_keyboard()
            .expect("Seat does not have a pointer")
    }
}
