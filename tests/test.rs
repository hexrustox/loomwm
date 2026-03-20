use loomwm::{config::Config, utils::get_app_id_and_title};

use crate::common::{
    TestState, assert_tiling_windows_title, done, get_active_workspace, has_n_windows, is_focused,
    run_compositor_test, spawn_alacritty, wait_until,
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

#[test]
fn test_single_window_spawn() {
    let handle = run_compositor_test(
        default_config(),
        (),
        wait_until(|data| has_n_windows(data, 1), Box::new(|_| {})),
    );

    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn test_focused_window_receives_keyboard() {
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
fn test_one_floating_one_tiling() {
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
    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn test_three_tiled_windows_with_titles() {
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
    }

    handle.join().unwrap();
}

#[test]
fn test_close_window_keeps_focus_on_remaining() {
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
    }

    handle.join().unwrap();
}
