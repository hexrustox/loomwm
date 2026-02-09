use serde::Deserialize;

use crate::{
    input::{
        KeyBindings, PointerBindings, ResizeLocation, test_key_bindings, test_pointer_bindings,
    },
    monitor::{LayoutSet, test_layout_set},
    window::rule::{WindowRules, test_window_rules},
};

#[derive(Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub window_rules: WindowRules,
    pub layout: LayoutConfig,
    pub key: KeyConfig,
    pub pointer: PointerConfig,
}

#[derive(Deserialize)]
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

#[derive(Deserialize)]
pub struct LayoutConfig {
    pub layouts: LayoutSet,
    pub default: String,
}

#[derive(Deserialize)]
pub struct KeyConfig {
    pub bindings: KeyBindings,
}

#[derive(Deserialize)]
pub struct PointerConfig {
    pub bindings: PointerBindings,
    pub resize: ResizeLocation,
}

// TEMP
pub fn test_config() -> Config {
    Config {
        general: GeneralConfig::default(),
        window_rules: test_window_rules(),
        layout: LayoutConfig {
            layouts: test_layout_set(),
            default: "master".to_string(),
        },
        key: KeyConfig {
            bindings: test_key_bindings(),
        },
        pointer: PointerConfig {
            bindings: test_pointer_bindings(),
            resize: ResizeLocation::Corner,
        },
    }
}
