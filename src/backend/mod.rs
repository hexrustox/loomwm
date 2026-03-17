use smithay::reexports::calloop::LoopHandle;

use crate::{
    CompositorData,
    backend::{headless::Headless, winit::Winit},
    state::WindowManagerState,
};

pub mod headless;
pub mod winit;

pub enum Backend {
    Headless(Headless),
    Winit(Box<Winit>),
}

impl Backend {
    pub fn new(event_loop: LoopHandle<CompositorData>) -> anyhow::Result<Self> {
        Ok(Self::Winit(Box::new(
            Winit::new(event_loop).map_err(|e| anyhow::anyhow!("{e}"))?,
        )))
    }
    pub fn winit(&mut self) -> &mut Winit {
        match self {
            Self::Winit(x) => x,
            _ => panic!("Backend is not winit"),
        }
    }

    pub fn headless(&mut self) -> &mut Headless {
        match self {
            Self::Headless(x) => x,
            _ => panic!("Backend is not headless"),
        }
    }

    pub fn init(&mut self, data: &mut WindowManagerState) {
        match self {
            Self::Winit(winit) => winit.init(data),
            Self::Headless(headless) => headless.init(),
        }
    }

    pub fn render(&mut self, data: &mut WindowManagerState) {
        match self {
            Self::Winit(winit) => winit.render(data),
            Self::Headless(headless) => headless.render(),
        }
    }
}
