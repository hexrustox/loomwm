#![allow(irrefutable_let_patterns)]

mod handlers;

mod grabs;
mod input;
mod layout;
mod state;
mod types;
mod window;
mod winit;

use anyhow::anyhow;
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use state::Smallvil;

use crate::winit::Winit;

pub struct CallLoopData {
    state: Smallvil,
    backend: Winit,
}

fn main() -> anyhow::Result<()> {
    let mut event_loop: EventLoop<CallLoopData> = EventLoop::try_new()?;

    let display: Display<Smallvil> = Display::new()?;
    let mut state = Smallvil::new(event_loop.handle(), event_loop.get_signal(), display);
    let mut backend = Winit::new(event_loop.handle()).map_err(|e| anyhow!("{e}"))?;
    backend.init(&mut state);

    let mut data = CallLoopData { state, backend };

    event_loop.run(None, &mut data, move |_| {
        // Smallvil is running
    })?;

    Ok(())
}
