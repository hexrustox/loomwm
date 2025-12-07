mod winit;

pub use winit::Winit;

use crate::state::WaylandState;

pub enum Backend {
    Winit(Winit),
}

impl Backend {
    pub fn winit(&mut self) -> Option<&mut Winit> {
        match self {
            Self::Winit(w) => Some(w),
        }
    }

    pub fn init(&mut self, compositor: &mut WaylandState) {
        match self {
            Self::Winit(w) => w.init(compositor),
        }
    }
}
