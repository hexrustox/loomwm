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
    state::WaylandState,
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

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum KeyAction {
    SwitchWorkspace {
        name: WorkspaceName,
    },
    MoveToWorkspace {
        name: WorkspaceName,
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

#[derive(Debug, Clone, Copy, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "lowercase")]
pub enum WindowDirection {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
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

// TEMP
pub fn test_key_bindings() -> KeyBindings {
    HashMap::from_iter([
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT,
                key: Keysym::_1,
            },
            KeyAction::SwitchWorkspace {
                name: WorkspaceName::Id(1),
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT,
                key: Keysym::_2,
            },
            KeyAction::SwitchWorkspace {
                name: WorkspaceName::Id(2),
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::SHIFT,
                key: Keysym::_1,
            },
            KeyAction::MoveToWorkspace {
                name: WorkspaceName::Id(1),
                focus: true,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::SHIFT,
                key: Keysym::_2,
            },
            KeyAction::MoveToWorkspace {
                name: WorkspaceName::Id(2),
                focus: true,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::CTRL,
                key: Keysym::_1,
            },
            KeyAction::MoveToWorkspace {
                name: WorkspaceName::Id(1),
                focus: false,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::CTRL,
                key: Keysym::_2,
            },
            KeyAction::MoveToWorkspace {
                name: WorkspaceName::Id(2),
                focus: false,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT,
                key: Keysym::space,
            },
            KeyAction::ToggleFloating,
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT,
                key: Keysym::t,
            },
            KeyAction::Execute {
                command: vec!["alacritty".to_string()],
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT,
                key: Keysym::q,
            },
            KeyAction::CloseWindow,
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT,
                key: Keysym::h,
            },
            KeyAction::FocusWindow {
                direction: WindowDirection::Left,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT,
                key: Keysym::j,
            },
            KeyAction::FocusWindow {
                direction: WindowDirection::Bottom,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT,
                key: Keysym::k,
            },
            KeyAction::FocusWindow {
                direction: WindowDirection::Top,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT,
                key: Keysym::l,
            },
            KeyAction::FocusWindow {
                direction: WindowDirection::Right,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::SHIFT,
                key: Keysym::h,
            },
            KeyAction::SwapWindow {
                direction: WindowDirection::Left,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::SHIFT,
                key: Keysym::j,
            },
            KeyAction::SwapWindow {
                direction: WindowDirection::Bottom,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::SHIFT,
                key: Keysym::k,
            },
            KeyAction::SwapWindow {
                direction: WindowDirection::Top,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::SHIFT,
                key: Keysym::l,
            },
            KeyAction::SwapWindow {
                direction: WindowDirection::Right,
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::CTRL,
                key: Keysym::h,
            },
            KeyAction::ResizeWindow {
                edge: WindowDirection::Left,
                unit: WindowUnit::Px(50),
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::CTRL,
                key: Keysym::j,
            },
            KeyAction::ResizeWindow {
                edge: WindowDirection::Bottom,
                unit: WindowUnit::Px(50),
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::CTRL,
                key: Keysym::k,
            },
            KeyAction::ResizeWindow {
                edge: WindowDirection::Top,
                unit: WindowUnit::Px(50),
            },
        ),
        (
            KeyCombo {
                modifiers: KeyModifiers::ALT | KeyModifiers::CTRL,
                key: Keysym::l,
            },
            KeyAction::ResizeWindow {
                edge: WindowDirection::Right,
                unit: WindowUnit::Px(50),
            },
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
                let bind = KeyCombo {
                    modifiers: data.key_modifiers,
                    key,
                };
                let pressed = event.state() == KeyState::Pressed;

                if pressed && let Some(action) = data.key_config.bindings.get(&bind) {
                    use KeyAction::*;
                    match action {
                        SwitchWorkspace { name } => {
                            data.switch_to_workspace(name.clone());
                        }
                        MoveToWorkspace { name, focus } => {
                            data.move_focused_window_to_workspace(name.clone(), *focus);
                        }
                        FocusWindow { direction } => {
                            data.focus_window_in_direction(*direction);
                        }
                        SwapWindow { direction } => {
                            data.swap_window_in_direction(*direction);
                        }
                        ResizeWindow { edge, unit } => {
                            data.resize_window_in_edge(*edge, *unit);
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

    #[derive(Deserialize)]
    struct T {
        x: KeyCombo,
    }

    #[test_case(r#""t""#, KeyModifiers::empty(), KEY_t; "single key")]
    #[test_case(r#""Ctrl+Shift+Return""#, KeyModifiers::CTRL | KeyModifiers::SHIFT, KEY_Return; "with modifiers")]
    fn test_deserialize_key(input: &str, modifiers: KeyModifiers, keysym: u32) {
        let kb = toml::from_str::<T>(&("x = ".to_string() + input)).unwrap();

        assert!(kb.x.modifiers.contains(modifiers));
        assert_eq!(kb.x.key.raw(), keysym);
    }

    #[test_case(r#""""#; "empty")]
    #[test_case(r#""hello""#; "unknown key")]
    #[test_case(r#""a+b""#; "multiple key")]
    #[test_case(r#""Super""#; "modifier only")]
    fn test_deserialize_key_fail(input: &str) {
        assert!(toml::from_str::<T>(&("x = ".to_string() + input)).is_err());
    }

    #[test_case(r#"{ action = "switch_workspace", name = 1 }"#)]
    #[test_case(r#"{ action = "focus_window", direction = "top" }"#)]
    #[test_case(r#"{ action = "resize_window", edge = "right", unit = "10px" }"#)]
    #[test_case(r#"{ action = "execute", command = [] }"#)]
    fn test_deserialize_action(input: &str) {
        toml::from_str::<KeyBindings>(&("x = ".to_string() + input)).unwrap();
    }
}
