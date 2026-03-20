mod integration_test {
    use std::{
        process::{Command, Stdio},
        sync::mpsc::channel,
        thread::{sleep, spawn},
        time::Duration,
    };

    use loomwm::{
        CompositorData, backend::Backend, config::Config, state::WindowManagerState,
        utils::get_app_id_and_title,
    };
    use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

    fn setup_compositor(config: Config) -> (EventLoop<'static, CompositorData>, CompositorData) {
        let event_loop = EventLoop::try_new().unwrap();
        let display = Display::new().unwrap();
        let mut backend = Backend::new_headless();
        let mut compositor = WindowManagerState::new(
            event_loop.handle(),
            event_loop.get_signal(),
            display,
            config,
        );
        backend.headless().init();
        backend.headless().add_output(&mut compositor, (0, 0));

        unsafe {
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
            std::env::set_var("WAYLAND_DISPLAY", &compositor.socket_name);
        }

        let data = CompositorData {
            compositor,
            backend,
            watcher: None,
        };

        (event_loop, data)
    }

    fn run_event_loop<F>(
        mut event_loop: EventLoop<'static, CompositorData>,
        mut data: CompositorData,
        condition: F,
    ) where
        F: Fn(&mut CompositorData) -> bool,
    {
        event_loop
            .run(None, &mut data, |data| {
                if condition(data) {
                    data.compositor.event_signal.stop();
                } else {
                    data.refresh_windows_and_flush_clients();
                }
            })
            .unwrap();
    }

    fn assert_window_count(data: &CompositorData, expected: usize) -> bool {
        let m = data.compositor.monitors.get_monitor();
        m.get_workspace(m.get_active_workspace_name())
            .windows_count()
            == expected
    }

    fn assert_windows_tiled(data: &CompositorData, expected_titles: &[usize]) -> bool {
        if !assert_window_count(data, expected_titles.len()) {
            return false;
        }

        let m = data.compositor.monitors.get_monitor();
        let iter = m
            .get_workspace(m.get_active_workspace_name())
            .tiling_windows_iter();

        for (w, &expected_title) in iter.zip(expected_titles) {
            let (_, t) = get_app_id_and_title(&w.wl_surface());
            assert_eq!(t, expected_title.to_string());
        }
        true
    }

    #[test]
    fn new_window() {
        let (s_s, s_r) = channel();

        let h = spawn(move || {
            let (event_loop, data) = setup_compositor(Config::default());
            s_s.send(()).unwrap();
            run_event_loop(event_loop, data, |d| assert_window_count(d, 1));
        });

        s_r.recv().unwrap();
        spawn(move || {
            let _ = Command::new("alacritty").stderr(Stdio::null()).spawn();
        });

        h.join().unwrap();
    }

    #[test]
    fn multiple_new_tiling_windows() {
        let (s_s, s_r) = channel();

        let h = spawn(move || {
            let config = toml::from_str::<Config>(
                r#"[layouts]
main = { nodes = [{ repeat = 3 }] }
default = "main"
"#,
            )
            .unwrap();
            let (event_loop, data) = setup_compositor(config);
            s_s.send(()).unwrap();
            run_event_loop(event_loop, data, |d| assert_windows_tiled(d, &[0, 1, 2]));
        });

        s_r.recv().unwrap();
        for i in 0..3 {
            spawn(move || {
                let _ = Command::new("alacritty")
                    .args(["-T", &i.to_string()])
                    .stderr(Stdio::null())
                    .spawn();
            });
            sleep(Duration::from_millis(100));
        }

        h.join().unwrap();
    }

    #[test]
    fn tiling_layout_fallback() {
        let (s_s, s_r) = channel();

        let h = spawn(move || {
            let config = toml::from_str::<Config>(
                r#"[layouts]
main = { nodes = [{ }] }
default = "main"
"#,
            )
            .unwrap();
            let (event_loop, data) = setup_compositor(config);
            s_s.send(()).unwrap();
            run_event_loop(event_loop, data, |d| {
                let m = d.compositor.monitors.get_monitor();
                let w = m.get_workspace(m.get_active_workspace_name());
                w.floating_windows_iter().count() == 1 && w.tiling_windows_iter().count() == 1
            });
        });

        s_r.recv().unwrap();
        for _ in 0..2 {
            spawn(move || {
                let _ = Command::new("alacritty").stderr(Stdio::null()).spawn();
            });
        }

        h.join().unwrap();
    }
}
