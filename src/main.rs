use std::{env, fs::read_to_string};

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

use crate::{
    backend::{Backend, Winit},
    state::WaylandState,
};

mod backend;
mod config;
mod handlers;
mod input;
mod monitor;
mod state;
mod utils;
mod window;

pub struct CompositorData {
    pub compositor: WaylandState,
    pub backend: Backend,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_file = read_to_string("/data/example/config.toml")?;
    let config = toml::from_str(&config_file)?;

    let mut event_loop: EventLoop<CompositorData> = EventLoop::try_new()?;
    let display: Display<WaylandState> = Display::new()?;
    let mut backend = Backend::Winit(Winit::new(event_loop.handle())?);
    let mut compositor = WaylandState::new(
        event_loop.handle(),
        event_loop.get_signal(),
        display,
        config,
    );
    backend.init(&mut compositor);

    unsafe {
        env::set_var("WAYLAND_DISPLAY", &compositor.socket_name);
    }
    let mut data = CompositorData {
        compositor,
        backend,
    };

    event_loop.run(None, &mut data, |_| {})?;

    Ok(())
}
