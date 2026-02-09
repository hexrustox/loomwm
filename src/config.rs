use serde::Deserialize;

use crate::{
    input::{KeyBindings, PointerBindings, ResizeLocation},
    monitor::LayoutSet,
    window::rule::WindowRules,
};

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub window_rules: WindowRules,
    pub layout: LayoutConfig,
    pub key: KeyConfig,
    pub pointer: PointerConfig,
}

#[derive(Deserialize)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
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
    pub layouts: LayoutSet,
    pub default: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct KeyConfig {
    pub bindings: KeyBindings,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct PointerConfig {
    pub bindings: PointerBindings,
    pub resize: ResizeLocation,
}
