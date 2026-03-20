use std::{
    cell::RefCell,
    sync::mpsc::channel,
    thread::{JoinHandle, spawn},
    time::{Duration, Instant},
};

use loomwm::{
    CompositorData, backend::Backend, config::Config, state::WindowManagerState,
    utils::get_app_id_and_title,
};
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

type DoneAssertion = Box<dyn Fn(&mut CompositorData)>;

pub enum TestState<C> {
    Running(C),
    Done(DoneAssertion),
}

pub fn run_compositor_test<C>(
    config: Config,
    initial_state: C,
    update: impl FnMut(&mut CompositorData, C) -> TestState<C> + Send + 'static,
) -> JoinHandle<()>
where
    C: Clone + Send + 'static,
{
    let (s_s, s_r) = channel();

    let h = spawn(move || {
        let (event_loop, data) = setup_compositor(config);
        s_s.send(()).unwrap();
        run_event_loop(event_loop, data, initial_state, update);
    });

    s_r.recv().unwrap();
    h
}

pub fn run_event_loop<C>(
    mut event_loop: EventLoop<'static, CompositorData>,
    mut data: CompositorData,
    initial_state: C,
    mut update: impl FnMut(&mut CompositorData, C) -> TestState<C>,
) where
    C: Clone,
{
    let end = Instant::now() + Duration::from_millis(1000);
    let state = RefCell::new(TestState::Running(initial_state));

    event_loop
        .run(None, &mut data, |data| {
            if Instant::now() >= end {
                panic!("Timeout");
            }

            let new_state = match &*state.borrow() {
                TestState::Done(assert_fn) => {
                    assert_fn(data);
                    data.compositor.event_signal.stop();
                    return;
                }
                TestState::Running(c) => update(data, c.clone()),
            };
            *state.borrow_mut() = new_state;

            data.refresh_windows_and_flush_clients();
        })
        .unwrap();
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
