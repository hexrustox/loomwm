use std::collections::HashMap;

use bitflags::bitflags;
use smithay::{
    backend::input::{Event, InputBackend, KeyState, KeyboardKeyEvent},
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
    SwitchWorkspace { name: u8 },
    MoveToWorkspace { name: u8, focus: bool },
    ToggleFloating,
}

// TEMP
pub fn test_key_bindings() -> KeyBindings {
    HashMap::from_iter([
        (
            KeyBinding {
                modifiers: KeyModifiers::ALT,
                key: Keysym::_1,
            },
            KeyAction::SwitchWorkspace { name: 1 },
        ),
        (
            KeyBinding {
                modifiers: KeyModifiers::ALT,
                key: Keysym::_2,
            },
            KeyAction::SwitchWorkspace { name: 2 },
        ),
        (
            KeyBinding {
                modifiers: KeyModifiers::ALT | KeyModifiers::SHIFT,
                key: Keysym::_1,
            },
            KeyAction::MoveToWorkspace {
                name: 1,
                focus: true,
            },
        ),
        (
            KeyBinding {
                modifiers: KeyModifiers::ALT | KeyModifiers::SHIFT,
                key: Keysym::_2,
            },
            KeyAction::MoveToWorkspace {
                name: 2,
                focus: true,
            },
        ),
        (
            KeyBinding {
                modifiers: KeyModifiers::ALT,
                key: Keysym::space,
            },
            KeyAction::ToggleFloating,
        ),
    ])
}

impl WaylandState {
    pub fn process_keyboard_event<B: InputBackend, T: KeyboardKeyEvent<B>>(&mut self, event: T) {
        let serial = SERIAL_COUNTER.next_serial();
        let time = Event::time_msec(&event);

        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard.input::<(), _>(
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

                let key = keysym_handle.raw_syms().swap_remove(0);
                let bind = KeyBinding {
                    modifiers: data.key_modifiers,
                    key,
                };
                let pressed = event.state() == KeyState::Pressed;

                if pressed && let Some(action) = data.key_config.bindings.get(&bind) {
                    use KeyAction::*;
                    match action {
                        SwitchWorkspace { name } => {
                            data.monitors.get_monitor_mut().switch_workspace(*name);
                        }
                        MoveToWorkspace { name, focus } => {
                            if let Some(wl_surface) = keyboard.current_focus() {
                                data.monitors.get_monitor_mut().move_window_to_workspace(
                                    &wl_surface,
                                    *name,
                                    *focus,
                                );
                            }
                        }
                        ToggleFloating => {
                            if let Some(wl_surface) = keyboard.current_focus() {
                                data.monitors
                                    .get_monitor_mut()
                                    .get_active_workspace()
                                    .toggle_window_floating(&wl_surface);
                            }
                        }
                    }
                    return FilterResult::Intercept(());
                }

                FilterResult::Forward
            },
        );
    }
}
