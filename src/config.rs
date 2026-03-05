use anyhow::anyhow;
use evdev::KeyCode;
use serde::{
    Deserialize, Deserializer,
    de::{self},
};
use smithay::input::keyboard::xkb;
use std::{collections::HashMap, fmt, fs::read_to_string, path::PathBuf, str::FromStr};

use crate::{
    input::{
        KeyBindings, KeyCombo, KeyModifiers, PointerBindings, PointerCombo, ResizeLocation,
        WindowUnit,
    },
    monitor::LayoutSet,
    path::config_dir,
    utils::Direction,
    window::rule::{WindowLocation, WindowRules},
};

// TODO run command at start up
#[derive(Deserialize, Default)]
#[serde(default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub general: GeneralConfig,
    pub window_rules: WindowRules,
    pub layouts: LayoutConfig,
    pub key: KeyConfig,
    pub pointer: PointerConfig,
    pub assistant: AssistantConfig,
}

impl Config {
    pub fn path() -> PathBuf {
        config_dir().join("config.toml")
    }

    pub fn read() -> anyhow::Result<Self> {
        let config_path = Self::path();
        toml::from_str(&read_to_string(&config_path)?)
            .map_err(|e| anyhow!("{e}").context("Failed to read config file"))
    }
}

#[derive(Deserialize)]
#[serde(default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GeneralConfig {
    pub allow_move_request: bool,
    pub allow_resize_request: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            allow_move_request: true,
            allow_resize_request: true,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    #[serde(flatten)]
    pub layout_set: LayoutSet,
    pub default: String,
}

#[derive(Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct KeyConfig {
    pub repeat_delay: u32,
    pub repeat_rate: u32,
    pub bindings: KeyBindings,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            repeat_delay: 200,
            repeat_rate: 25,
            bindings: KeyBindings::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PointerConfig {
    pub bindings: PointerBindings,
    #[serde(rename = "resize-at")]
    pub resize: ResizeLocation,
    pub selection: f32,
}

impl Default for PointerConfig {
    fn default() -> Self {
        Self {
            bindings: HashMap::default(),
            resize: ResizeLocation::default(),
            selection: 0.8,
        }
    }
}

#[derive(Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct AssistantConfig {
    pub enable: bool,
    pub save_layout_after: u64,
    pub history_length: usize,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            enable: true,
            save_layout_after: 300,
            history_length: usize::MAX,
        }
    }
}

impl<'de> Deserialize<'de> for Direction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Direction;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("one of the following flags: top, bottom, left, right, top_left, bottom_left, top_right, bottom_right")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Direction::from_name(&value.to_uppercase())
                    .ok_or(E::invalid_value(de::Unexpected::Str(value), &self))
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl<'de> Deserialize<'de> for WindowLocation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = WindowLocation;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(r#""center" or [x, y]"#)
            }

            fn visit_str<E>(self, value: &str) -> Result<WindowLocation, E>
            where
                E: de::Error,
            {
                match value.to_lowercase().as_str() {
                    "center" => Ok(WindowLocation::Center),
                    _ => Err(de::Error::invalid_value(de::Unexpected::Str(value), &self)),
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<WindowLocation, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                const EXP: &&str = &"a sequence of length 2";
                let x = seq
                    .next_element::<i32>()?
                    .ok_or_else(|| de::Error::invalid_length(0, EXP))?;
                let y = seq
                    .next_element::<i32>()?
                    .ok_or_else(|| de::Error::invalid_length(1, EXP))?;

                if seq.next_element::<de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::invalid_length(3, EXP));
                }

                Ok(WindowLocation::Location(x, y))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

impl<'de> Deserialize<'de> for WindowUnit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = WindowUnit;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a size unit")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                v.strip_suffix("px")
                    .and_then(|px| px.parse().ok())
                    .map(|px: i32| WindowUnit::Px(px))
                    .ok_or(E::custom("invalid pixel"))
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(WindowUnit::Ratio(v))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(WindowUnit::Ratio(v as f64))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(WindowUnit::Ratio(v as f64))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

fn deserialize_combo<'de, K, T, D>(
    deserializer: D,
    expecting: &str,
    parse_key: impl FnMut(&str) -> Result<K, String>,
    construct: impl FnOnce(KeyModifiers, K) -> T,
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor<F, G> {
        expecting: String,
        parse_key: F,
        construct: G,
    }

    impl<'de, T, K, F, G> de::Visitor<'de> for Visitor<F, G>
    where
        F: FnMut(&str) -> Result<K, String>,
        G: FnOnce(KeyModifiers, K) -> T,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str(&self.expecting)
        }

        fn visit_str<E>(mut self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let parts: Vec<&str> = v.split('+').map(|s| s.trim()).collect();

            let (key_part, mod_parts) = parts
                .split_last()
                .filter(|(part, _)| !part.is_empty())
                .ok_or_else(|| E::invalid_value(de::Unexpected::Str(v), &self))?;

            let modifiers = parse_modifiers(mod_parts)?;

            let key = (self.parse_key)(key_part).map_err(E::custom)?;

            Ok((self.construct)(modifiers, key))
        }
    }

    deserializer.deserialize_str(Visitor {
        expecting: expecting.to_string(),
        parse_key,
        construct,
    })
}

impl<'de> Deserialize<'de> for KeyCombo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_combo(
            deserializer,
            "a keybinding string",
            |s| {
                let key = xkb::keysym_from_name(s, xkb::KEYSYM_CASE_INSENSITIVE);
                if key == xkb::Keysym::NoSymbol {
                    Err(format!("{} is not a valid key name", s))
                } else {
                    Ok(key)
                }
            },
            KeyCombo::new,
        )
    }
}

impl<'de> Deserialize<'de> for PointerCombo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_combo(
            deserializer,
            "a pointer binding string",
            |s| {
                KeyCode::from_str(&s.to_uppercase())
                    .map_err(|_| format!("{} is not a valid key code", s))
            },
            PointerCombo::new,
        )
    }
}

fn parse_modifiers<E>(mod_parts: &[&str]) -> Result<KeyModifiers, E>
where
    E: de::Error,
{
    let mut modifiers = KeyModifiers::empty();

    for &m in mod_parts {
        match m.to_lowercase().as_str() {
            "ctrl" => modifiers |= KeyModifiers::CTRL,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            "alt" => modifiers |= KeyModifiers::ALT,
            "super" => modifiers |= KeyModifiers::SUPER,
            _ => {
                return Err(E::invalid_value(
                    de::Unexpected::Str(m),
                    &"a valid modifier (ctrl, shift, alt, super)",
                ));
            }
        }
    }

    Ok(modifiers)
}

#[cfg(test)]
mod tests {
    use evdev::KeyCode;
    use test_case::test_case;
    use xkbcommon::xkb::Keysym;

    use super::*;
    use crate::input::{KeyCombo, KeyModifiers, PointerCombo};
    use crate::utils::Direction;
    use crate::window::rule::WindowLocation;

    #[test_case("top" => Direction::TOP; "top")]
    #[test_case("bottom" => Direction::BOTTOM; "bottom")]
    #[test_case("left" => Direction::LEFT; "left")]
    #[test_case("right" => Direction::RIGHT; "right")]
    #[test_case("top_left" => Direction::TOP_LEFT; "top left")]
    #[test_case("bottom_left" => Direction::BOTTOM_LEFT; "bottom left")]
    #[test_case("top_right" => Direction::TOP_RIGHT; "top right")]
    #[test_case("bottom_right" => Direction::BOTTOM_RIGHT; "bottom right")]
    fn test_direction_valid(s: &str) -> Direction {
        #[derive(Deserialize)]
        struct Wrapper {
            direction: Direction,
        }
        toml::from_str::<Wrapper>(&format!("direction = \"{}\"", s))
            .unwrap()
            .direction
    }

    #[test]
    fn test_direction_invalid() {
        #[derive(Deserialize, Debug)]
        struct Wrapper {
            #[allow(unused)]
            direction: Direction,
        }
        toml::from_str::<Wrapper>("direction = \"invalid\"").unwrap_err();
    }

    #[test_case("\"center\"" => WindowLocation::Center; "center")]
    #[test_case("[100, 200]" => WindowLocation::Location(100, 200); "array")]
    fn test_window_location_valid(s: &str) -> WindowLocation {
        #[derive(Deserialize)]
        struct Wrapper {
            location: WindowLocation,
        }
        toml::from_str::<Wrapper>(&format!("location = {}", s))
            .unwrap()
            .location
    }

    #[test_case("\"invalid\""; "invalid string")]
    #[test_case("[1]"; "wrong length 1")]
    #[test_case("[1, 2, 3]"; "wrong length 3")]
    fn test_window_location_invalid(s: &str) {
        #[derive(Deserialize, Debug)]
        struct Wrapper {
            #[allow(unused)]
            location: WindowLocation,
        }
        toml::from_str::<Wrapper>(&format!("location = {}", s)).unwrap_err();
    }

    #[test_case("\"10px\"" => WindowUnit::Px(10))]
    #[test_case("1" => WindowUnit::Ratio(1.0))]
    #[test_case("2.0" => WindowUnit::Ratio(2.0))]
    fn test_window_unit_valid(s: &str) -> WindowUnit {
        #[derive(Deserialize)]
        struct Wrapper {
            unit: WindowUnit,
        }
        toml::from_str::<Wrapper>(&format!("unit = {}", s))
            .unwrap()
            .unit
    }

    #[test_case("a" => KeyCombo::new(KeyModifiers::empty(), Keysym::a); "no modifier")]
    #[test_case("alt+1" => KeyCombo::new(KeyModifiers::ALT, Keysym::_1); "single modifier")]
    #[test_case("ctrl+shift+a" => KeyCombo::new(KeyModifiers::CTRL | KeyModifiers::SHIFT, Keysym::a); "multiple modifiers")]
    #[test_case("super+space" => KeyCombo::new(KeyModifiers::SUPER, Keysym::space); "super modifier")]
    fn test_key_combo_valid(s: &str) -> KeyCombo {
        #[derive(Deserialize, Debug)]
        struct Wrapper {
            key: KeyCombo,
        }
        toml::from_str::<Wrapper>(&format!("key = \"{}\"", s))
            .unwrap()
            .key
    }

    #[test_case(""; "empty string")]
    #[test_case("meta+1"; "unknown modifier")]
    #[test_case("unknown"; "unknown key")]
    #[test_case("+"; "separator only")]
    #[test_case("ctrl+"; "trailing separator")]
    fn test_key_combo_invalid(s: &str) {
        #[derive(Deserialize, Debug)]
        struct Wrapper {
            #[allow(unused)]
            key: KeyCombo,
        }
        toml::from_str::<Wrapper>(&format!("key = \"{}\"", s)).unwrap_err();
    }

    #[test_case("alt+btn_left" => PointerCombo::new(KeyModifiers::ALT, KeyCode::BTN_LEFT); "left button")]
    #[test_case("ctrl+btn_right" => PointerCombo::new(KeyModifiers::CTRL, KeyCode::BTN_RIGHT); "right button with ctrl")]
    #[test_case("super+btn_middle" => PointerCombo::new(KeyModifiers::SUPER, KeyCode::BTN_MIDDLE); "middle button")]
    fn test_pointer_combo_valid(s: &str) -> PointerCombo {
        #[derive(Deserialize, Debug)]
        struct Wrapper {
            btn: PointerCombo,
        }
        toml::from_str::<Wrapper>(&format!("btn = \"{}\"", s))
            .unwrap()
            .btn
    }

    #[test_case(""; "empty string")]
    #[test_case("meta+btn_left"; "unknown modifier")]
    #[test_case("alt+btn_invalid"; "unknown button")]
    fn test_pointer_combo_invalid(s: &str) {
        #[derive(Deserialize, Debug)]
        struct Wrapper {
            #[allow(unused)]
            btn: PointerCombo,
        }
        toml::from_str::<Wrapper>(&format!("btn = \"{}\"", s)).unwrap_err();
    }
}
