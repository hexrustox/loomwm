use std::{collections::HashMap, fmt, process::Command};

use bitflags::bitflags;
use serde::{
    Deserialize, Deserializer,
    de::{self, IntoDeserializer},
};
use smithay::{
    backend::input::{Event, InputBackend, KeyState, KeyboardKeyEvent},
    input::keyboard::{FilterResult, Keysym},
    utils::SERIAL_COUNTER,
};
use xkbcommon::xkb::{self, keysyms::KEY_NoSymbol};

use crate::{
    monitor::{TileRatio, WorkspaceName},
    state::WindowManagerState,
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

pub type KeyBindings = HashMap<KeyCombo, KeyAction>;

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct KeyCombo {
    modifiers: KeyModifiers,
    key: Keysym,
}

impl<'de> Deserialize<'de> for KeyCombo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct KeyComboVisitor;

        impl<'de> de::Visitor<'de> for KeyComboVisitor {
            type Value = KeyCombo;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a keybinding string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let parts: Vec<&str> = v.split('+').map(|s| s.trim()).collect();

                if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
                    return Err(E::custom("empty keybinding"));
                }

                let mut modifiers = KeyModifiers::empty();

                let (key_part, mod_parts) = parts.split_last().unwrap();

                for &m in mod_parts {
                    match m.to_lowercase().as_str() {
                        "ctrl" => modifiers |= KeyModifiers::CTRL,
                        "shift" => modifiers |= KeyModifiers::SHIFT,
                        "alt" => modifiers |= KeyModifiers::ALT,
                        "super" => modifiers |= KeyModifiers::SUPER,
                        _ => return Err(E::custom(format!("unknown modifier: {}", m))),
                    }
                }

                let key = xkb::keysym_from_name(key_part, xkb::KEYSYM_CASE_INSENSITIVE);

                if key.raw() == KEY_NoSymbol {
                    return Err(E::custom(format!("unknown key: {}", key_part)));
                }

                Ok(KeyCombo { modifiers, key })
            }
        }

        deserializer.deserialize_str(KeyComboVisitor)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum KeyAction {
    SwitchWorkspace {
        name: WorkspaceName,
    },
    MoveToWorkspace {
        name: WorkspaceName,
        #[serde(default = "default_focus")]
        focus: bool,
    },
    FocusWindow {
        direction: WindowDirection,
    },
    SwapWindow {
        direction: WindowDirection,
    },
    ResizeWindow {
        edge: WindowDirection,
        unit: WindowUnit,
    },
    ToggleFloating,
    CloseWindow,
    Execute {
        command: Vec<String>,
    },
}

fn default_focus() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WindowDirection {
    Up,
    Down,
    Left,
    Right,
}

impl WindowDirection {
    pub fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowUnit {
    Ratio(TileRatio),
    Px(i32),
}

impl<'de> Deserialize<'de> for WindowUnit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct WindowUnitVisitor;

        impl<'de> de::Visitor<'de> for WindowUnitVisitor {
            type Value = WindowUnit;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a size unit")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if let Some(px) = v.strip_suffix("px") {
                    Ok(WindowUnit::Px(
                        px.parse().map_err(|_| E::custom("invalid pixel"))?,
                    ))
                } else {
                    Ok(WindowUnit::Ratio(TileRatio::deserialize(
                        v.into_deserializer(),
                    )?))
                }
            }
        }

        deserializer.deserialize_str(WindowUnitVisitor)
    }
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
                let key = keysym_handle.raw_syms().swap_remove(0);
                let bind = KeyCombo {
                    modifiers: data.key_modifiers,
                    key,
                };
                let pressed = event.state() == KeyState::Pressed;

                if pressed && let Some(action) = data.key_config.bindings.get(&bind) {
                    use KeyAction::*;
                    match action {
                        SwitchWorkspace { name } => {
                            data.change_or_create_active_workspace(name.clone());
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
                        ResizeWindow { edge, unit } => {
                            data.resize_focused_tiling_window_in_edge(*edge, *unit);
                        }
                        ToggleFloating => {
                            data.toggle_focused_window_floating();
                        }
                        CloseWindow => {
                            data.close_focused_window();
                        }
                        Execute { command: args } => {
                            if let Some(program) = args.first() {
                                // TODO
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

#[cfg(test)]
mod tests {
    use xkbcommon::xkb::keysyms::*;

    use super::*;
    use test_case::test_case;

    #[derive(Debug, Deserialize, PartialEq)]
    struct T {
        k: Option<KeyCombo>,
        a: Option<KeyAction>,
    }

    #[test_case(r#""t""#, KeyModifiers::empty(), KEY_t; "single key")]
    #[test_case(r#""Ctrl+Shift+Return""#, KeyModifiers::CTRL | KeyModifiers::SHIFT, KEY_Return; "with modifiers")]
    fn test_deserialize_key(input: &str, modifiers: KeyModifiers, keysym: u32) {
        assert_eq!(
            toml::from_str::<T>(&("k = ".to_string() + input)).unwrap(),
            T {
                k: Some(KeyCombo {
                    modifiers,
                    key: keysym.into()
                }),
                a: None
            }
        );
    }

    #[test_case(r#""""#; "empty")]
    #[test_case(r#""hello""#; "unknown key")]
    #[test_case(r#""a+b""#; "multiple key")]
    #[test_case(r#""Super""#; "modifier only")]
    fn test_deserialize_key_fail(input: &str) {
        assert!(toml::from_str::<T>(&("k = ".to_string() + input)).is_err());
    }

    #[test_case(r#"{ action = "switch_workspace", name = 1 }"#, KeyAction::SwitchWorkspace { name: WorkspaceName::Id(1) })]
    #[test_case(r#"{ action = "focus_window", direction = "up" }"#, KeyAction::FocusWindow { direction: WindowDirection::Up })]
    #[test_case(r#"{ action = "resize_window", edge = "right", unit = "10px" }"#, KeyAction::ResizeWindow { edge: WindowDirection::Right, unit: WindowUnit::Px(10) })]
    #[test_case(r#"{ action = "execute", command = [] }"#, KeyAction::Execute { command: vec![] })]
    fn test_deserialize_action(input: &str, expected: KeyAction) {
        assert_eq!(
            toml::from_str::<T>(&("a = ".to_string() + input)).unwrap(),
            T {
                k: None,
                a: Some(expected)
            }
        );
    }
}
