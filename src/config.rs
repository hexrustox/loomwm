use crate::{
    input::{PointerBindings, ResizeLocation, test_pointer_bindings},
    workspace::{LayoutSet, test_layout_set},
};

pub struct Config {
    pub layout: LayoutConfig,
    pub pointer: PointerConfig,
}

pub struct LayoutConfig {
    pub layouts: LayoutSet,
    pub default: String,
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
        pointer: PointerConfig {
            bindings: test_pointer_bindings(),
            resize: ResizeLocation::Corner,
        },
    }
}
