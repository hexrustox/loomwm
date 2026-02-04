use std::collections::HashMap;

use bitflags::bitflags;
use smithay::{
    backend::input::{Event, InputBackend, KeyboardKeyEvent},
    input::keyboard::{FilterResult, Keysym},
    utils::SERIAL_COUNTER,
};

use crate::state::WaylandState;

bitflags! {
    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
    pub struct KeyModifiers: u32 {
        const CTRL =  0b0001;
        const SHIFT = 0b0010;
        const ALT =   0b0100;
        const SUPER = 0b1000;
    }
}

pub type KeyBindings = HashMap<KeyBinding, KeyAction>;

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct KeyBinding {
    modifiers: KeyModifiers,
    key: Keysym,
}

pub enum KeyAction {
    SwitchWorkspace(u8),
}

// TEMP
pub fn test_key_bindings() -> KeyBindings {
    HashMap::from_iter([
        (
            KeyBinding {
                modifiers: KeyModifiers::ALT,
                key: Keysym::_1,
            },
            KeyAction::SwitchWorkspace(1),
        ),
        (
            KeyBinding {
                modifiers: KeyModifiers::ALT,
                key: Keysym::_2,
            },
            KeyAction::SwitchWorkspace(2),
        ),
    ])
}

impl WaylandState {
    pub fn process_keyboard_event<B: InputBackend, T: KeyboardKeyEvent<B>>(&mut self, event: T) {
        let serial = SERIAL_COUNTER.next_serial();
        let time = Event::time_msec(&event);

        self.seat.get_keyboard().unwrap().input::<(), _>(
            self,
            event.key_code(),
            event.state(),
            serial,
            time,
            |data, modifiers_state, keysym_handle| {
                data.key_modifiers = {
                    let mut key_modifiers = KeyModifiers::empty();
                    if modifiers_state.ctrl {
                        key_modifiers |= KeyModifiers::CTRL;
                    }
                    if modifiers_state.shift {
                        key_modifiers |= KeyModifiers::SHIFT;
                    }
                    if modifiers_state.alt {
                        key_modifiers |= KeyModifiers::ALT;
                    }
                    if modifiers_state.logo {
                        key_modifiers |= KeyModifiers::SUPER;
                    }
                    key_modifiers
                };

                let key = keysym_handle.modified_sym();
                if let Some(action) = data.key_config.bindings.get(&KeyBinding {
                    modifiers: data.key_modifiers,
                    key,
                }) {
                    use KeyAction::*;
                    match action {
                        SwitchWorkspace(n) => {
                            data.workspaces.switch_workspace(*n);
                        }
                    }
                    return FilterResult::Intercept(());
                }

                FilterResult::Forward
            },
        );
    }
}
