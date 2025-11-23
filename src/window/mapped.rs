use smithay::desktop::Window;

#[derive(Debug)]
pub struct MappedWindow {
    pub window: Window,
    is_focused: bool,
}

impl MappedWindow {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            is_focused: false,
        }
    }
}
