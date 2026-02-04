use std::rc::Rc;

use smithay::output::Output;

use crate::monitor::workspace::{LayoutSet, Monitor};

pub mod workspace;

#[derive(Default)]
pub struct Monitors {
    monitors: Vec<Monitor>,
}

impl Monitors {
    pub fn push(&mut self, output: Output, layouts: Rc<LayoutSet>, layout_name: Rc<str>) {
        self.monitors
            .push(Monitor::new(output, layouts, layout_name));
    }

    // TEMP
    pub fn get_monitor(&self) -> &Monitor {
        self.monitors.last().unwrap()
    }
    pub fn get_monitor_mut(&mut self) -> &mut Monitor {
        self.monitors.last_mut().unwrap()
    }
}
