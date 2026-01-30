use std::env;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

use crate::{
    backend::{Backend, Winit},
    state::WaylandState,
};

mod backend;
mod handlers;
mod input;
mod state;
mod utils;
mod window;
mod workspace;

pub struct AppState {
    pub compositor: WaylandState,
    pub backend: Backend,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<AppState> = EventLoop::try_new()?;
    let display: Display<WaylandState> = Display::new()?;
    let mut backend = Backend::Winit(Winit::new(event_loop.handle())?);
    let mut compositor = WaylandState::new(event_loop.handle(), event_loop.get_signal(), display);
    backend.init(&mut compositor);

    unsafe {
        env::set_var("WAYLAND_DISPLAY", &compositor.socket_name);
    }
    let mut state = AppState {
        compositor,
        backend,
    };

    std::process::Command::new("alacritty").spawn().ok();
    std::process::Command::new("alacritty").spawn().ok();
    std::process::Command::new("alacritty").spawn().ok();
    std::process::Command::new("alacritty").spawn().ok();

    event_loop.run(None, &mut state, |_| {})?;

    Ok(())
}
