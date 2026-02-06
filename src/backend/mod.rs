mod winit;

pub use winit::Winit;

use crate::state::WaylandState;

pub enum Backend {
    Winit(Winit),
}

impl Backend {
    pub fn winit(&mut self) -> Option<&mut Winit> {
        match self {
            Self::Winit(winit) => Some(winit),
        }
    }

    pub fn init(&mut self, compositor: &mut WaylandState) {
        match self {
            Self::Winit(winit) => winit.init(compositor),
        }
    }
}
