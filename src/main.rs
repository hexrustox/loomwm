#![recursion_limit = "256"]

use std::env;

use anyhow::anyhow;
use notify::INotifyWatcher;
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use tracing::level_filters::LevelFilter;

use crate::{
    backend::{Backend, Winit},
    config::Config,
    state::WindowManagerState,
};

mod backend;
mod config;
mod handlers;
mod input;
mod monitor;
mod path;
mod state;
mod utils;
mod watcher;
mod window;

pub struct CompositorData {
    pub compositor: WindowManagerState,
    pub backend: Backend,
    pub watcher: Option<INotifyWatcher>,
}
fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .init();

    let config = Config::read()?;
    let mut event_loop: EventLoop<CompositorData> = EventLoop::try_new()?;
    let display: Display<WindowManagerState> = Display::new()?;
    let mut backend = Backend::Winit(Winit::new(event_loop.handle()).map_err(|e| anyhow!("{e}"))?);
    let mut compositor = WindowManagerState::new(
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
        watcher: watcher::init(&event_loop),
    };

    event_loop.run(None, &mut data, |data| {
        data.compositor.refresh();

        data.backend.render(&mut data.compositor);
        data.compositor.send_frame_to_windows();

        data.compositor.display_handle.flush_clients().unwrap();
    })?;

    Ok(())
}
