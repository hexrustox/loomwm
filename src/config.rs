// TODO more tests
use serde::{Deserialize, Deserializer, de};
use std::fmt;

use crate::{
    input::{KeyBindings, PointerBindings, ResizeLocation},
    monitor::LayoutSet,
    state::WindowManagerState,
    utils::Direction,
    window::rule::WindowRules,
};

#[derive(Deserialize, Default)]
#[serde(default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub general: GeneralConfig,
    pub window_rules: WindowRules,
    pub layouts: LayoutConfig,
    pub key: KeyConfig,
    pub pointer: PointerConfig,
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

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct KeyConfig {
    pub bindings: KeyBindings,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PointerConfig {
    pub bindings: PointerBindings,
    #[serde(rename = "resize-at")]
    pub resize: ResizeLocation,
    #[serde(default = "default_selection")]
    pub selection: f32,
}

fn default_selection() -> f32 {
    0.8
}

impl<'de> Deserialize<'de> for Direction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ResizeEdgeVisitor;

        impl<'de> de::Visitor<'de> for ResizeEdgeVisitor {
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

        deserializer.deserialize_str(ResizeEdgeVisitor)
    }
}

impl WindowManagerState {
    pub fn update_config(&mut self, config: Config) {
        if self.window_rules.get_hash() != config.window_rules.get_hash() {
            self.window_rules = config.window_rules;
            self.apply_rule_to_mapped_windows();
        }

        self.general_config = config.general;
        // TODO live reload
        // self.layout_set = Rc::new(config.layouts.layout_set);
        // self.default_layout = Rc::from(config.layouts.default);
        self.key_config = config.key;
        self.pointer_config = config.pointer;
    }
}
