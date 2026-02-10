use smithay::{
    backend::input::{InputBackend, InputEvent},
    input::{keyboard::KeyboardHandle, pointer::PointerHandle},
};

use crate::state::WindowManagerState;

pub use keyboard::{KeyBindings, KeyModifiers, WindowDirection, WindowUnit};
pub use pointer::{PointerBindings, ResizeLocation};

pub mod floating_resize_grab;
mod keyboard;
pub mod move_grab;
mod pointer;
mod swap_grab;
mod tiling_resize_grab;

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
