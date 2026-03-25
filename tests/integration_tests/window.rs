use loomwm::utils::get_app_id_and_title;

use crate::integration_tests::common::{
    TestState, active_workspace_has_n_windows, assert_tiling_windows_title, default_config, done,
    get_floating_window, get_floating_windows_count, get_tiling_windows_count, get_window,
    is_focused, run_compositor_test, spawn_alacritty, tiling_config, wait_until,
};

#[test]
fn test_single_window_spawn() {
    let handle = run_compositor_test(
        default_config(),
        (),
        wait_until(
            |data| active_workspace_has_n_windows(data, 1),
            Box::new(|_| {}),
        ),
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
            |data| active_workspace_has_n_windows(data, 1),
            Box::new(|data| {
                let mapped = get_floating_window(data);
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
            |data| active_workspace_has_n_windows(data, 2),
            Box::new(|data| {
                assert_eq!(get_floating_windows_count(data), 1);
                assert_eq!(get_tiling_windows_count(data), 1);
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
            |data| active_workspace_has_n_windows(data, 3),
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

    let handle = run_compositor_test(default_config(), 0, move |data, phase| match phase {
        0 if active_workspace_has_n_windows(data, 2) => {
            assert_tiling_windows_title(data, titles.iter().map(|n| n.to_string()).collect());
            let mapped = get_window(data);
            assert!(is_focused(data, mapped.wl_surface()));
            let (_, title) = get_app_id_and_title(&mapped.wl_surface());
            assert_eq!(title, "1");

            data.compositor.close_focused_window();
            TestState::Running(1)
        }
        1 if active_workspace_has_n_windows(data, 1) => done(|data| {
            let mapped = get_window(data);
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
