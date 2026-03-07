mod winit;

pub use winit::Winit;

use crate::state::WindowManagerState;

pub enum Backend {
    Winit(Winit),
}

impl Backend {
    pub fn winit(&mut self) -> &mut Winit {
        match self {
            Self::Winit(x) => x,
        }
    }

    pub fn init(&mut self, data: &mut WindowManagerState) {
        match self {
            Self::Winit(winit) => winit.init(data),
        }
    }

    pub fn render(&mut self, data: &mut WindowManagerState) {
        match self {
            Self::Winit(winit) => winit.render(data),
        }
    }
}
