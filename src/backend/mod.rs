mod winit;

pub use winit::Winit;

pub enum Backend {
    Winit(Winit),
}

impl Backend {
    pub fn winit(&mut self) -> Option<&mut Winit> {
        match self {
            Self::Winit(w) => Some(w),
        }
    }
}
