use std::{
    cell::RefCell,
    process::{Command, Stdio},
    sync::mpsc::channel,
    thread::{JoinHandle, sleep, spawn},
    time::{Duration, Instant},
};

use loomwm::{
    CompositorData,
    backend::Backend,
    config::Config,
    monitor::{Workspace, WorkspaceName},
    state::WindowManagerState,
    utils::get_app_id_and_title,
};
use smithay::reexports::{
    calloop::EventLoop, wayland_server::Display, wayland_server::protocol::wl_surface::WlSurface,
};

type DoneAssertion = Box<dyn Fn(&mut CompositorData) + Send>;

pub enum TestState<C> {
    Running(C),
    Done(DoneAssertion),
}

pub fn default_config() -> Config {
    Config::default()
}

pub fn tiling_config(layout: &str) -> Config {
    toml::from_str::<Config>(&format!(
        r#"[layouts]
main = {{ {} }}
default = "main"
"#,
        layout
    ))
    .unwrap()
}

pub fn run_compositor_test<C>(
    config: Config,
    initial_state: C,
    update: impl FnMut(&mut CompositorData, C) -> TestState<C> + Send + 'static,
) -> JoinHandle<()>
where
    C: Clone + Send + 'static,
{
    let (tx, rx) = channel();

    let handle = spawn(move || {
        let (event_loop, data) = setup_compositor(config);
        tx.send(()).unwrap();
        run_event_loop(event_loop, data, initial_state, update);
    });

    rx.recv().unwrap();
    handle
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
    backend.headless().add_output(&mut compositor, (100, 100));

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

pub fn run_event_loop<C>(
    mut event_loop: EventLoop<'static, CompositorData>,
    mut data: CompositorData,
    initial_state: C,
    mut update: impl FnMut(&mut CompositorData, C) -> TestState<C>,
) where
    C: Clone,
{
    let deadline = Instant::now() + Duration::from_millis(1000);
    let state = RefCell::new(TestState::Running(initial_state));

    event_loop
        .run(None, &mut data, |data| {
            if Instant::now() >= deadline {
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

pub fn spawn_alacritty(title: Option<String>) {
    spawn(move || {
        let mut cmd = Command::new("alacritty");
        cmd.stderr(Stdio::null());
        if let Some(t) = title {
            cmd.args(["-T", &t]);
        }
        let _ = cmd.spawn();
    });
    sleep(Duration::from_millis(50));
}

pub fn wait_until<C>(
    check: impl Fn(&CompositorData) -> bool + 'static,
    assertion: DoneAssertion,
) -> impl FnMut(&mut CompositorData, C) -> TestState<C> {
    let assertion = RefCell::new(Some(assertion));
    move |data, state| {
        if check(data) {
            let assertion = assertion.borrow_mut().take().unwrap();
            TestState::Done(assertion)
        } else {
            TestState::Running(state)
        }
    }
}

pub fn done<C>(f: impl Fn(&mut CompositorData) + Send + 'static) -> TestState<C> {
    TestState::Done(Box::new(f))
}

pub fn get_active_workspace(data: &loomwm::CompositorData) -> &Workspace {
    let monitor = data.compositor.monitors.get_monitor();
    monitor.get_workspace(monitor.get_active_workspace_name())
}

pub fn active_workspace_has_n_windows(data: &CompositorData, expected: usize) -> bool {
    get_active_workspace(data).windows_count() == expected
}

pub fn workspace_has_n_windows(
    data: &CompositorData,
    workspace_name: WorkspaceName,
    count: usize,
) -> bool {
    data.compositor
        .monitors
        .get_monitor()
        .get_workspace(&workspace_name)
        .windows_count()
        == count
}

pub fn get_next_window_in_workspace(
    data: &CompositorData,
    workspace_name: WorkspaceName,
) -> &loomwm::window::MappedWindow {
    let monitor = data.compositor.monitors.get_monitor();
    monitor
        .get_workspace(&workspace_name)
        .windows_iter()
        .next()
        .unwrap()
}

pub fn get_floating_window(data: &CompositorData) -> loomwm::window::MappedWindow {
    get_active_workspace(data)
        .floating_windows_iter()
        .next()
        .unwrap()
        .clone()
}

pub fn get_tiling_window(data: &CompositorData, index: usize) -> loomwm::window::MappedWindow {
    get_active_workspace(data)
        .tiling_windows_iter()
        .nth(index)
        .unwrap()
        .clone()
}

pub fn is_focused(data: &CompositorData, surface: WlSurface) -> bool {
    let current = data.compositor.get_keyboard().current_focus();
    current.is_some_and(|s| s == surface)
}

pub fn get_active_workspace_name(data: &CompositorData) -> &WorkspaceName {
    data.compositor
        .monitors
        .get_monitor()
        .get_active_workspace_name()
}

pub fn assert_active_workspace_name(data: &CompositorData, expected: WorkspaceName) {
    assert_eq!(get_active_workspace_name(data), &expected);
}

pub fn assert_window_focused_in_workspace(data: &CompositorData, workspace_name: WorkspaceName) {
    let window = get_next_window_in_workspace(data, workspace_name);
    assert!(is_focused(data, window.wl_surface()));
}

pub fn assert_tiling_windows_title(data: &CompositorData, expected_titles: Vec<String>) {
    let iter = get_active_workspace(data).tiling_windows_iter();

    for (mapped, expected_title) in iter.zip(expected_titles) {
        let surface = mapped.wl_surface();
        let (_, title) = get_app_id_and_title(&surface);
        assert_eq!(title, expected_title);
    }
}
