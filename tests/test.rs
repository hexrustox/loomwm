use std::{thread::sleep, time::Duration};

use loomwm::{config::Config, utils::get_app_id_and_title};

use crate::common::{
    TestState, assert_tiling_windows_title, done, has_n_windows, is_focused, run_compositor_test,
    spawn_alacritty, wait_until,
};

mod common;

fn default_config() -> Config {
    Config::default()
}

fn tiling_config(layout: &str) -> Config {
    toml::from_str::<Config>(&format!(
        r#"[layouts]
main = {{ {} }}
default = "main"
"#,
        layout
    ))
    .unwrap()
}

fn get_active_workspace(data: &loomwm::CompositorData) -> &loomwm::monitor::Workspace {
    let monitor = data.compositor.monitors.get_monitor();
    monitor.get_workspace(monitor.get_active_workspace_name())
}

#[test]
fn todo_1() {
    let handle = run_compositor_test(
        default_config(),
        (),
        wait_until(|data| has_n_windows(data, 1), Box::new(|_| {})),
    );

    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn todo_2() {
    let titles = [0, 1, 2];

    let handle = run_compositor_test(
        tiling_config("nodes = [{ repeat = 3 }]"),
        (),
        wait_until(
            |data| has_n_windows(data, 3),
            Box::new(move |data| {
                assert_tiling_windows_title(data, titles.iter().map(|n| n.to_string()).collect());
            }),
        ),
    );

    for title in titles {
        spawn_alacritty(Some(title.to_string()));
        sleep(Duration::from_millis(50));
    }

    handle.join().unwrap();
}

#[test]
fn todo_3() {
    let handle = run_compositor_test(
        tiling_config("nodes = [{ }]"),
        (),
        wait_until(
            |data| has_n_windows(data, 2),
            Box::new(|data| {
                let workspace = get_active_workspace(data);
                assert!(workspace.floating_windows_iter().count() == 1);
                assert!(workspace.tiling_windows_iter().count() == 1);
            }),
        ),
    );

    spawn_alacritty(None);
    sleep(Duration::from_millis(50));
    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn todo_4() {
    let handle = run_compositor_test(
        default_config(),
        (),
        wait_until(
            |data| has_n_windows(data, 1),
            Box::new(|data| {
                let workspace = get_active_workspace(data);
                let mapped = workspace.floating_windows_iter().next().unwrap();
                assert!(is_focused(data, mapped.wl_surface()));
            }),
        ),
    );

    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn todo_5() {
    let titles = [0, 1];

    let handle = run_compositor_test(default_config(), 0u8, move |data, phase| match phase {
        0 if has_n_windows(data, 2) => {
            assert_tiling_windows_title(data, titles.iter().map(|n| n.to_string()).collect());
            let workspace = get_active_workspace(data);
            let mapped = workspace.floating_windows_iter().next().unwrap();
            assert!(is_focused(data, mapped.wl_surface()));
            let (_, title) = get_app_id_and_title(&mapped.wl_surface());
            assert_eq!(title, "1");

            data.compositor.close_focused_window();
            TestState::Running(1)
        }
        1 if has_n_windows(data, 1) => done(|data| {
            let workspace = get_active_workspace(data);
            let mapped = workspace.floating_windows_iter().next().unwrap();
            assert!(is_focused(data, mapped.wl_surface()));
            let (_, title) = get_app_id_and_title(&mapped.wl_surface());
            assert_eq!(title, "0");
        }),
        _ => TestState::Running(phase),
    });

    for title in titles {
        spawn_alacritty(Some(title.to_string()));
        sleep(Duration::from_millis(50));
    }

    handle.join().unwrap();
}
