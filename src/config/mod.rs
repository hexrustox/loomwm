use anyhow::anyhow;
use serde::Deserialize;
use std::{fs::read_to_string, path::PathBuf};

use crate::{
    input::{KeyBindings, PointerBindings, ResizeLocation},
    monitor::LayoutSet,
    path::config_dir,
    window::rule::WindowRules,
};

mod parse;

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
        let content = read_to_string(Self::path())?;
        toml::from_str(&content).map_err(|e| anyhow!("{e}"))
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

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PointerConfig {
    pub bindings: PointerBindings,
    #[serde(rename = "resize-at")]
    pub resize: ResizeLocation,
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
            history_length: 10,
        }
    }
}
