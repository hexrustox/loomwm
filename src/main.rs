use std::env;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

use crate::{
    backend::{Backend, Winit},
    state::WMState,
};

mod backend;
mod handlers;
mod state;
mod window;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<WMState> = EventLoop::try_new()?;
    let display: Display<WMState> = Display::new()?;
    let backend = Backend::Winit(Winit::new(event_loop.handle())?);
    let mut state = WMState::new(
        event_loop.handle(),
        event_loop.get_signal(),
        display,
        backend,
    );

    unsafe {
        env::set_var("WAYLAND_DISPLAY", &state.socket_name);
    }

    std::process::Command::new("alacritty").spawn().ok();

    event_loop.run(None, &mut state, |_| {})?;

    Ok(())
}
