use smithay::backend::input::{InputBackend, InputEvent};

use crate::state::WindowManagerState;

pub use keyboard::{KeyBindings, KeyModifiers, WindowDirection, WindowUnit};
pub use pointer::{PointerBindings, ResizeLocation};

mod keyboard;
pub mod move_grab;
mod pointer;
pub mod resize_grab;
pub mod swap_grab;

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
}
