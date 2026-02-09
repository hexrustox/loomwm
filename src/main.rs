use std::{env, fs::read_to_string, path::Path};

use notify::{Event, EventKind, Watcher, event::ModifyKind};
use smithay::reexports::{
    calloop::{EventLoop, channel},
    wayland_server::Display,
};

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

const CONFIG: &str = "/data/example/config.toml";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_file = read_to_string(CONFIG)?;
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

    let (sender, reciever) = channel::channel();

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
        Path::new(CONFIG).parent().unwrap(),
        notify::RecursiveMode::NonRecursive,
    )?;

    event_loop
        .handle()
        .insert_source(reciever, |event, _, data| {
            if let channel::Event::Msg(_) = event
                && let Ok(file) = read_to_string(CONFIG)
                && let Ok(config) = toml::from_str(&file)
            {
                data.compositor.update_config(config);
            }
        })?;

    event_loop.run(None, &mut data, |_| {})?;

    Ok(())
}
