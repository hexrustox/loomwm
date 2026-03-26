use std::{collections::HashMap, process::Command, time::Duration};

use bitflags::bitflags;
use serde::Deserialize;
use smithay::{
    backend::input::{Event, InputBackend, KeyState, KeyboardKeyEvent},
    input::keyboard::{FilterResult, Keysym},
    reexports::calloop::timer::{TimeoutAction, Timer},
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
#[derive(Debug, Deserialize, PartialEq, Clone)]
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
    ToggleFloating {
        value: Option<bool>,
    },
    ToggleFloatingHidden {
        value: Option<bool>,
        // TODO use serde default
        focus: Option<bool>,
    },
    ToggleFullscreen {
        value: Option<bool>,
    },
    CloseWindow,
    Execute {
        command: Vec<String>,
    },
    Assistant,
    // TODO focus last focused workspace/window, reset windows' ratio in layout
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

                let Some(key) = keysym_handle.raw_syms().first().cloned() else {
                    return FilterResult::Forward;
                };

                let bind = KeyCombo {
                    modifiers: data.key_modifiers,
                    key,
                };
                let pressed = event.state() == KeyState::Pressed;

                if let Some(action) = data.key_config.bindings.get(&bind) {
                    if pressed {
                        let action = data.handle_action(action.clone());
                        let is_none = data.repeat_action.is_none();
                        data.repeat_action = Some(action);
                        if is_none {
                            let _ = data.event_loop.insert_source(
                                Timer::from_duration(Duration::from_millis(
                                    data.key_config.repeat_delay as u64,
                                )),
                                |_, _, data| {
                                    let data = &mut data.compositor;
                                    if let Some(ref action) = data.repeat_action {
                                        data.handle_action(action.clone());
                                        TimeoutAction::ToDuration(Duration::from_millis(
                                            (1000 / data.key_config.repeat_rate) as u64,
                                        ))
                                    } else {
                                        TimeoutAction::Drop
                                    }
                                },
                            );
                        }
                    } else if data.repeat_action.as_ref().is_some_and(|a| a == action) {
                        data.repeat_action = None;
                    }
                    return FilterResult::Intercept(());
                }

                FilterResult::Forward
            },
        );
    }

    pub fn handle_action(&mut self, action: KeyAction) -> KeyAction {
        use KeyAction::*;
        match &action {
            PrevWorkspace => {
                self.goto_prev_workspace();
            }
            NextWorkspace => {
                self.goto_next_workspace();
            }
            SwitchWorkspace { name } => {
                self.switch_or_create_active_workspace(name.clone());
            }
            MoveToWorkspace { name, focus } => {
                self.move_focused_window_to_workspace(name.clone(), *focus);
            }
            FocusWindow { direction } => {
                self.focus_tiling_window_in_direction(*direction);
            }
            SwapWindow { direction } => {
                self.swap_focused_tiling_window_in_direction(*direction);
            }
            ResizeWindow { direction, unit } => {
                self.resize_focused_tiling_window(*direction, *unit);
            }
            ToggleFloating { value } => {
                self.toggle_focused_window_floating(*value);
            }
            ToggleFloatingHidden { value, focus } => {
                self.set_focused_workspace_floating_window_hidden(*value, *focus);
            }
            ToggleFullscreen { value } => {
                if let Some(surface) = self.get_keyboard().current_focus() {
                    self.fullscreen_window(&surface, *value);
                }
            }
            CloseWindow => {
                self.close_focused_window();
            }
            Execute { command: args } => {
                // TODO dump stdout & err
                if let Some(program) = args.first() {
                    let _ = Command::new(program).args(args.iter().skip(1)).spawn();
                }
            }
            Assistant => {
                self.run_assistant();
            }
        }

        action
    }
}
