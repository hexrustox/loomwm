use std::{collections::HashMap, process::Command};

use bitflags::bitflags;
use serde::Deserialize;
use smithay::{
    backend::input::{Event, InputBackend, KeyState, KeyboardKeyEvent},
    input::keyboard::{FilterResult, Keysym},
    utils::SERIAL_COUNTER,
};

use crate::{
    monitor::{TileRatio, WorkspaceName},
    state::WindowManagerState,
    utils::Direction,
};

bitflags! {
    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
    pub struct KeyModifiers: u32 {
        const CTRL =  0b0001;
        const SHIFT = 0b0010;
        const ALT =   0b0100;
        const SUPER = 0b1000;
    }
}

// TODO multi action per key, sub key bind
pub type KeyBindings = HashMap<KeyCombo, KeyAction>;

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct KeyCombo {
    modifiers: KeyModifiers,
    key: Keysym,
}

impl KeyCombo {
    pub fn new(modifiers: KeyModifiers, key: Keysym) -> Self {
        Self { modifiers, key }
    }
}

// TODO aggregate with mouse action
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum KeyAction {
    PrevWorkspace,
    NextWorkspace,
    SwitchWorkspace {
        name: WorkspaceName,
    },
    MoveToWorkspace {
        name: WorkspaceName,
        #[serde(default = "default_focus")]
        focus: bool,
    },
    FocusWindow {
        direction: Direction,
    },
    SwapWindow {
        direction: Direction,
    },
    ResizeWindow {
        direction: Direction,
        unit: WindowUnit,
    },
    ToggleFloating,
    CloseWindow,
    Execute {
        command: Vec<String>,
    },
    // TODO last focus workspace/window
}

fn default_focus() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowUnit {
    Ratio(TileRatio),
    Px(i32),
}

impl WindowManagerState {
    pub fn process_keyboard_event<B: InputBackend, T: KeyboardKeyEvent<B>>(&mut self, event: T) {
        let serial = SERIAL_COUNTER.next_serial();
        let time = Event::time_msec(&event);

        let keyboard = self.get_keyboard();
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

                if data.get_pointer().is_grabbed() {
                    return FilterResult::Intercept(());
                }

                // TODO count key down
                let key = keysym_handle.modified_sym();
                let bind = KeyCombo {
                    modifiers: data.key_modifiers,
                    key,
                };
                let pressed = event.state() == KeyState::Pressed;

                if pressed && let Some(action) = data.key_config.bindings.get(&bind) {
                    use KeyAction::*;
                    match action {
                        PrevWorkspace => {
                            data.goto_prev_workspace();
                        }
                        NextWorkspace => {
                            data.goto_next_workspace();
                        }
                        SwitchWorkspace { name } => {
                            data.switch_or_create_active_workspace(name.clone());
                        }
                        MoveToWorkspace { name, focus } => {
                            data.move_focused_window_to_workspace(name.clone(), *focus);
                        }
                        FocusWindow { direction } => {
                            data.focus_tiling_window_in_direction(*direction);
                        }
                        SwapWindow { direction } => {
                            data.swap_focused_tiling_window_in_direction(*direction);
                        }
                        ResizeWindow { direction, unit } => {
                            data.resize_focused_tiling_window(*direction, *unit);
                        }
                        ToggleFloating => {
                            data.toggle_focused_window_floating();
                        }
                        CloseWindow => {
                            data.close_focused_window();
                        }
                        Execute { command: args } => {
                            if let Some(program) = args.first() {
                                let _ = Command::new(program).args(args.iter().skip(1)).spawn();
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
