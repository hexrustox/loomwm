use smithay::backend::input::{InputBackend, InputEvent};

use crate::state::WaylandState;

pub use keyboard::KeyModifiers;
pub use pointer::{PointerBindings, test_pointer_bindings};

mod keyboard;
pub mod move_grab;
mod pointer;
pub mod resize_grab;

impl WaylandState {
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
