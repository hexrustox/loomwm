use std::{
    process::Command,
    sync::mpsc::channel,
    thread::{sleep, spawn},
    time::Duration,
};

use loomwm::{CompositorData, backend::Backend, config::Config, state::WindowManagerState};
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

#[test]
fn new_window() {
    let (s_s, s_r) = channel();
    let (c_s, c_r) = channel();

    let h = spawn(move || {
        let mut event_loop = EventLoop::try_new().unwrap();
        let display = Display::new().unwrap();
        let mut backend = Backend::new_headless();
        let mut compositor = WindowManagerState::new(
            event_loop.handle(),
            event_loop.get_signal(),
            display,
            Config::default(),
        );
        backend.headless().add_output(&mut compositor, (0, 0));

        unsafe {
            std::env::set_var("WAYLAND_DISPLAY", &compositor.socket_name);
        }

        s_s.send(()).unwrap();

        let mut data = CompositorData {
            compositor,
            backend,
            watcher: None,
        };

        event_loop
            .run(None, &mut data, |data| {
                if let Ok(()) = c_r.try_recv() {
                    data.compositor.event_signal.stop();
                }
            })
            .unwrap();
    });

    s_r.recv().unwrap();

    let mut child = Command::new("alacritty").spawn().unwrap();
    sleep(Duration::from_millis(100));

    c_s.send(()).unwrap();

    child.kill().ok();
    child.wait().ok();

    h.join().unwrap();
}
