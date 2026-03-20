use std::{
    sync::mpsc::channel,
    thread::{JoinHandle, spawn},
    time::{Duration, Instant},
};

use loomwm::{
    CompositorData, backend::Backend, config::Config, state::WindowManagerState,
    utils::get_app_id_and_title,
};
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

pub fn run_compositor_test<C, A>(config: Config, condition: C, assertion: A) -> JoinHandle<()>
where
    C: Fn(&mut CompositorData) -> bool + Send + 'static,
    A: Fn(&mut CompositorData) + Send + 'static,
{
    let (s_s, s_r) = channel();

    let h = spawn(move || {
        let (event_loop, data) = setup_compositor(config);
        s_s.send(()).unwrap();
        run_event_loop(event_loop, data, condition, assertion);
    });

    s_r.recv().unwrap();
    h
}

pub fn setup_compositor(config: Config) -> (EventLoop<'static, CompositorData>, CompositorData) {
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

pub fn run_event_loop<C, A>(
    mut event_loop: EventLoop<'static, CompositorData>,
    mut data: CompositorData,
    condition: C,
    assertion: A,
) where
    C: Fn(&mut CompositorData) -> bool,
    A: Fn(&mut CompositorData),
{
    let end = Instant::now() + Duration::from_millis(1000);
    event_loop
        .run(None, &mut data, |data| {
            if Instant::now() >= end {
                panic!("Timeout");
            }

            if condition(data) {
                assertion(data);
                data.compositor.event_signal.stop();
            } else {
                data.refresh_windows_and_flush_clients();
            }
        })
        .unwrap();
}

pub fn has_n_windows(data: &CompositorData, expected: usize) -> bool {
    let m = data.compositor.monitors.get_monitor();
    m.get_workspace(m.get_active_workspace_name())
        .windows_count()
        == expected
}

pub fn assert_tiling_windows_title(data: &CompositorData, expected_titles: Vec<String>) -> bool {
    let m = data.compositor.monitors.get_monitor();
    let iter = m
        .get_workspace(m.get_active_workspace_name())
        .tiling_windows_iter();

    for (w, expected_title) in iter.zip(expected_titles) {
        let (_, t) = get_app_id_and_title(&w.wl_surface());
        assert_eq!(t, expected_title);
    }
    true
}
