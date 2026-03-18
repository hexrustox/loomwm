use std::{
    process::{Command, Stdio},
    sync::mpsc::channel,
    thread::spawn,
};

use loomwm::{CompositorData, backend::Backend, config::Config, state::WindowManagerState};
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

#[test]
fn new_window() {
    let (s_s, s_r) = channel();

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
        backend.headless().init();
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
                let m = data.compositor.monitors.get_monitor();
                if m.get_workspace(m.get_active_workspace_name())
                    .windows_count()
                    == 1
                {
                    data.compositor.event_signal.stop();
                } else {
                    data.refresh_windows_and_flush_clients();
                }
            })
            .unwrap();
    });

    spawn(move || {
        s_r.recv().unwrap();
        let _ = Command::new("alacritty").stderr(Stdio::null()).status();
    });

    h.join().unwrap();
}
