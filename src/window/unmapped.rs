use smithay::{
    backend::renderer::utils::with_renderer_surface_state, desktop::Window,
    wayland::shell::xdg::ToplevelSurface,
};

#[derive(Debug)]
pub struct UnmappedWindow {
    pub window: Window,
    pub state: UnmappedWindowConfigureState,
}

#[derive(Debug, Default)]
pub enum UnmappedWindowConfigureState {
    #[default]
    NotConfigured,
    Configured,
}

impl UnmappedWindow {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            state: Default::default(),
        }
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.window.toplevel().expect("No X11 support")
    }

    pub fn is_mapped(&self) -> bool {
        with_renderer_surface_state(self.toplevel().wl_surface(), |state| {
            state.buffer().is_some()
        })
        .unwrap_or(false)
    }

    pub fn is_configured(&self) -> bool {
        matches!(self.state, UnmappedWindowConfigureState::Configured)
    }
}
