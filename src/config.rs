use crate::{
    input::{
        KeyBindings, PointerBindings, ResizeLocation, test_key_bindings, test_pointer_bindings,
    },
    workspace::{LayoutSet, test_layout_set},
};

pub struct Config {
    pub layout: LayoutConfig,
    pub key: KeyConfig,
    pub pointer: PointerConfig,
}

pub struct LayoutConfig {
    pub layouts: LayoutSet,
    pub default: String,
}

pub struct KeyConfig {
    pub bindings: KeyBindings,
}

pub struct PointerConfig {
    pub bindings: PointerBindings,
    pub resize: ResizeLocation,
}

// TEMP
pub fn test_config() -> Config {
    Config {
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
