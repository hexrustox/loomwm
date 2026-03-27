use std::{fmt, str::FromStr};

use evdev::KeyCode;
use serde::{Deserialize, Deserializer, de};
use smithay::input::keyboard::xkb;

use crate::{
    input::{KeyCombo, KeyModifiers, PointerCombo, WindowUnit},
    monitor::{TileRatio, WorkspaceName},
    utils::{Direction, RGBAColor},
    window::{
        WindowRole,
        rule::{
            WindowBorder, WindowDecoration, WindowDynamicProperties, WindowLocation,
            WindowOpeningProperties, WindowProperties, WindowRule, WindowRuleMatch, WindowState,
        },
    },
};

impl<'de> Deserialize<'de> for Direction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Direction;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("one of: top, bottom, left, right, top_left, bottom_left, top_right, bottom_right")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Direction::from_name(&value.to_uppercase()).ok_or_else(|| {
                    E::unknown_variant(
                        value,
                        &[
                            "top",
                            "bottom",
                            "left",
                            "right",
                            "top_left",
                            "bottom_left",
                            "top_right",
                            "bottom_right",
                        ],
                    )
                })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl<'de> Deserialize<'de> for RGBAColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = RGBAColor;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a hex string (RRGGBB or RRGGBBAA)")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if (v.len() == 6 || v.len() == 8)
                    && let Ok(num) = u32::from_str_radix(v, 16)
                {
                    return Ok(RGBAColor::new(if v.len() == 6 {
                        (num << 8) | 0xFF
                    } else {
                        num
                    }));
                }
                Err(E::invalid_value(de::Unexpected::Str(v), &self))
            }
        }

        deserializer.deserialize_any(Visitor)
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
                formatter.write_str("'center' or [x, y]")
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

impl<'de> Deserialize<'de> for WindowRole {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = WindowRole;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    formatter,
                    "one of: maximized, fullscreen, swap-source, swap-target"
                )
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<WindowRole, E> {
                match v {
                    "maximized" => Ok(WindowRole::Maximized),
                    "fullscreen" => Ok(WindowRole::Fullscreen),
                    "swap-source" => Ok(WindowRole::SwapSource),
                    "swap-target" => Ok(WindowRole::SwapTarget),
                    other => Err(E::unknown_variant(
                        other,
                        &["maximized", "fullscreen", "swap-source", "swap-target"],
                    )),
                }
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
struct WindowRuleMatchDe {
    app_id_pattern: Option<String>,
    title_pattern: Option<String>,
    is_focused: Option<bool>,
    is_floating: Option<bool>,
    is_maximized: Option<bool>,
    is_fullscreen: Option<bool>,
    is_swap_source: Option<bool>,
    is_swap_target: Option<bool>,
    in_workspace: Option<WorkspaceName>,
}

impl WindowRuleMatchDe {
    fn try_into_match(self) -> Result<WindowRuleMatch, String> {
        let mut set = Vec::new();
        if self.is_maximized == Some(true) {
            set.push("is-maximized");
        }
        if self.is_fullscreen == Some(true) {
            set.push("is-fullscreen");
        }
        if self.is_swap_source == Some(true) {
            set.push("is-swap-source");
        }
        if self.is_swap_target == Some(true) {
            set.push("is-swap-target");
        }
        if set.len() > 1 {
            return Err(format!(
                "conflicting fields set to true: {}. these are mutually exclusive",
                set.join(", ")
            ));
        }

        let role = if self.is_maximized == Some(true) {
            Some(WindowRole::Maximized)
        } else if self.is_fullscreen == Some(true) {
            Some(WindowRole::Fullscreen)
        } else if self.is_swap_source == Some(true) {
            Some(WindowRole::SwapSource)
        } else if self.is_swap_target == Some(true) {
            Some(WindowRole::SwapTarget)
        } else {
            None
        };

        Ok(WindowRuleMatch {
            app_id_pattern: self.app_id_pattern,
            title_pattern: self.title_pattern,
            is_focused: self.is_focused,
            is_floating: self.is_floating,
            role,
            in_workspace: self.in_workspace,
        })
    }
}

impl<'de> Deserialize<'de> for WindowRuleMatch {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let de = WindowRuleMatchDe::deserialize(deserializer)?;
        de.try_into_match().map_err(de::Error::custom)
    }
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
struct WindowRuleDe {
    matches: Vec<WindowRuleMatch>,
    open_with_focus: Option<bool>,
    open_as: Option<WindowRole>,
    open_as_floating: Option<FloatingConfig>,
    open_as_tiling: Option<TilingConfig>,
    open_in_workspace: Option<WorkspaceName>,
    decoration: Option<WindowDecoration>,
    border: Option<WindowBorder>,
    opacity: Option<f32>,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
struct FloatingConfig {
    location: Option<WindowLocation>,
    size: Option<(i32, i32)>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TilingConfig {
    ratio: Option<TileRatio>,
}

impl WindowRuleDe {
    fn try_into_rule(self) -> Result<WindowRule, String> {
        if self.open_as_floating.is_some() && self.open_as_tiling.is_some() {
            return Err("open-as-floating and open-as-tiling are mutually exclusive".into());
        }

        let layout_state = if let Some(f) = self.open_as_floating {
            Some(WindowState::Float {
                location: f.location,
                size: f.size,
            })
        } else {
            self.open_as_tiling
                .map(|t| WindowState::Tile { ratio: t.ratio })
        };

        Ok(WindowRule {
            matches: if self.matches.is_empty() {
                vec![WindowRuleMatch::default()]
            } else {
                self.matches
            },
            properties: WindowProperties {
                opening: Some(WindowOpeningProperties {
                    open_with_focus: self.open_with_focus,
                    layout_state,
                    open_as: self.open_as,
                    open_in_workspace: self.open_in_workspace,
                }),
                dynamic: WindowDynamicProperties {
                    decoration: self.decoration,
                    border: self.border,
                    opacity: self.opacity,
                },
            },
        })
    }
}

impl<'de> Deserialize<'de> for WindowRule {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let de = WindowRuleDe::deserialize(deserializer)?;
        de.try_into_rule().map_err(de::Error::custom)
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
                formatter.write_str(r#"a pixel string (e.g. "10px") or a ratio number"#)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                v.strip_suffix("px")
                    .and_then(|px| px.parse().ok())
                    .map(|px: i32| WindowUnit::Px(px))
                    .ok_or(E::invalid_value(de::Unexpected::Str(v), &self))
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
            "a keybinding string (e.g. 'ctrl+shift+a')",
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
            "a pointer binding string (e.g. 'alt+btn_left')",
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
                return Err(E::unknown_variant(m, &["ctrl", "shift", "alt", "super"]));
            }
        }
    }

    Ok(modifiers)
}

#[cfg(test)]
mod tests {
    use evdev::KeyCode;
    use smithay::input::keyboard::Keysym;
    use test_case::test_case;

    use super::*;
    use crate::config::Config;
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

    #[test_case("\"123456\"" => RGBAColor::new(0x123456FF))]
    #[test_case("\"12345678\"" => RGBAColor::new(0x12345678))]
    fn test_rgba_color_valid(s: &str) -> RGBAColor {
        #[derive(Deserialize)]
        struct Wrapper {
            color: RGBAColor,
        }
        toml::from_str::<Wrapper>(&format!("color = {}", s))
            .unwrap()
            .color
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

    #[test_case("\"maximized\"" => WindowRole::Maximized; "maximized")]
    #[test_case("\"fullscreen\"" => WindowRole::Fullscreen; "fullscreen")]
    #[test_case("\"swap-source\"" => WindowRole::SwapSource; "swap_source")]
    #[test_case("\"swap-target\"" => WindowRole::SwapTarget; "swap_target")]
    fn test_window_role_valid(s: &str) -> WindowRole {
        #[derive(Deserialize)]
        struct Wrapper {
            role: WindowRole,
        }
        toml::from_str::<Wrapper>(&format!("role = {}", s))
            .unwrap()
            .role
    }

    #[test_case("\"normal\""; "normal")]
    #[test_case("\"foo\""; "unknown")]
    #[test_case("\"MAXIMIZED\""; "wrong case")]
    fn test_window_role_invalid(s: &str) {
        #[derive(Deserialize, Debug)]
        struct Wrapper {
            #[allow(unused)]
            role: WindowRole,
        }
        toml::from_str::<Wrapper>(&format!("role = {}", s)).unwrap_err();
    }

    #[test_case("is-maximized = true" => Some(WindowRole::Maximized); "is_maximized")]
    #[test_case("is-fullscreen = true" => Some(WindowRole::Fullscreen); "is_fullscreen")]
    #[test_case("is-swap-source = true" => Some(WindowRole::SwapSource); "is_swap_source")]
    #[test_case("is-swap-target = true" => Some(WindowRole::SwapTarget); "is_swap_target")]
    fn test_window_rule_match_role(s: &str) -> Option<WindowRole> {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(flatten)]
            m: WindowRuleMatch,
        }
        toml::from_str::<Wrapper>(s).unwrap().m.role
    }

    #[test_case("" => None; "no fields")]
    #[test_case("is-maximized = false" => None; "false means unset")]
    #[test_case("is-fullscreen = false" => None; "fullscreen false")]
    fn test_window_rule_match_no_role(s: &str) -> Option<WindowRole> {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(flatten)]
            m: WindowRuleMatch,
        }
        toml::from_str::<Wrapper>(s).unwrap().m.role
    }

    #[test_case("is-maximized = true\nis-fullscreen = true"; "maximized_and_fullscreen")]
    #[test_case("is-swap-source = true\nis-swap-target = true"; "swap_source_and_swap_target")]
    #[test_case("is-maximized = true\nis-fullscreen = true\nis-swap-source = true"; "three at once")]
    fn test_window_rule_match_conflicting(s: &str) {
        #[derive(Deserialize, Debug)]
        struct Wrapper {
            #[allow(unused)]
            #[serde(flatten)]
            m: WindowRuleMatch,
        }
        toml::from_str::<Wrapper>(s).unwrap_err();
    }

    #[test]
    fn test_window_rule_match_deny_unknown() {
        let toml = r#"
            [[window-rules]]
            foo = true
        "#;
        toml::from_str::<Config>(toml).unwrap_err();
    }

    #[test_case("open-as = \"maximized\""; "maximized")]
    #[test_case("open-as = \"fullscreen\""; "fullscreen")]
    fn test_open_as_valid(s: &str) {
        let toml = format!("[[window-rules]]\n{s}");
        toml::from_str::<Config>(&toml).unwrap();
    }

    #[test_case("open-as = \"normal\""; "normal")]
    #[test_case("open-as = \"foo\""; "unknown")]
    fn test_open_as_invalid(s: &str) {
        let toml = format!("[[window-rules]]\n{s}");
        toml::from_str::<Config>(&toml).unwrap_err();
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
