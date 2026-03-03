#![recursion_limit = "256"]

use std::{env, fs::read_to_string};

use anyhow::anyhow;
use notify::{Event, EventKind, Watcher, event::ModifyKind};
use smithay::reexports::{
    calloop::{EventLoop, channel},
    wayland_server::Display,
};
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
mod window;

pub struct CompositorData {
    pub compositor: WindowManagerState,
    pub backend: Backend,
}

fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::WARN)
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
    };

    let (sender, reciever) = channel::channel();

    // TODO refactor
    let mut watcher = notify::recommended_watcher(move |event| {
        if let Ok(Event {
            kind: EventKind::Modify(ModifyKind::Data(..)),
            ..
        }) = event
        {
            sender.send(()).unwrap()
        }
    })?;

    watcher.watch(
        Config::path().parent().unwrap(),
        notify::RecursiveMode::NonRecursive,
    )?;

    event_loop
        .handle()
        .insert_source(reciever, move |event, _, data| {
            if let channel::Event::Msg(_) = event
                && let Ok(file) = read_to_string(Config::path())
                && let Ok(config) = toml::from_str(&file)
            {
                data.compositor.update_config(config);
            }
        })
        .map_err(|e| anyhow!("{e}"))?;

    event_loop.run(None, &mut data, |_| {})?;

    Ok(())
}
