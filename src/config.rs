use std::rc::Rc;

use crate::{
    input::{PointerBindings, ResizeLocation, test_pointer_bindings},
    workspace::{LayoutSet, test_layout_set},
};

pub struct Config {
    pub layouts: Rc<LayoutSet>,
    pub pointer: PointerConfig,
}

pub struct PointerConfig {
    pub bindings: PointerBindings,
    pub resize: ResizeLocation,
}

// TEMP
pub fn test_config() -> Config {
    Config {
        layouts: Rc::new(test_layout_set()),
        pointer: PointerConfig {
            bindings: test_pointer_bindings(),
            resize: ResizeLocation::Corner,
        },
    }
}
