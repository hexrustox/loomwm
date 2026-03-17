use smithay::{
    output::{Mode, Output, PhysicalProperties, Subpixel},
    utils::{Physical, Size},
};

use crate::state::WindowManagerState;

pub struct Headless {}

impl Headless {
    pub fn new() -> Self {
        Self {}
    }

    pub fn add_output(
        &mut self,
        data: &mut WindowManagerState,
        size: impl Into<Size<i32, Physical>>,
    ) {
        let output = Output::new(
            "headless".to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "Smithay".to_string(),
                model: "Headless".to_string(),
            },
        );

        let mode = Mode {
            size: size.into(),
            refresh: 60,
        };
        output.change_current_state(Some(mode), None, None, None);
        output.set_preferred(mode);

        data.space.map_output(&output, (0, 0));
        data.monitors
            .push(output, data.layout_set.clone(), data.default_layout.clone());
    }

    pub fn init(&self) {}
    pub fn render(&self) {}
}
