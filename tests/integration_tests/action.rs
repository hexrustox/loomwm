use std::cell::RefCell;

use loomwm::{
    input::WindowUnit,
    monitor::{TileTreeWindow, WorkspaceName},
    utils::Direction,
};

use crate::integration_tests::common::{
    TestState, active_workspace_has_n_windows, assert_active_workspace_name,
    assert_tiling_windows_title, assert_window_focused_in_workspace, default_config, done,
    get_active_workspace, get_floating_window, get_floating_windows_count,
    get_next_window_in_workspace, get_tiling_window, get_tiling_windows_count, is_focused,
    run_compositor_test, spawn_alacritty, tiling_config, workspace_has_n_windows,
};

#[test]
fn test_workspace_switch() {
    let handle = run_compositor_test(default_config(), 0, |data, phase| match phase {
        0 if active_workspace_has_n_windows(data, 1) => {
            data.compositor.switch_workspace(WorkspaceName::Id(2));
            TestState::Running(1)
        }
        1 => {
            assert_active_workspace_name(data, WorkspaceName::Id(2));
            spawn_alacritty(None);
            TestState::Running(2)
        }
        2 if active_workspace_has_n_windows(data, 1) => {
            assert_window_focused_in_workspace(data, WorkspaceName::Id(2));
            data.compositor.switch_workspace(WorkspaceName::Id(1));
            TestState::Running(3)
        }
        3 => {
            assert_window_focused_in_workspace(data, WorkspaceName::Id(1));
            data.compositor.switch_workspace(WorkspaceName::Id(2));
            TestState::Running(4)
        }
        4 => done(|data| {
            assert_window_focused_in_workspace(data, WorkspaceName::Id(2));
        }),
        _ => TestState::Running(phase),
    });

    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn test_move_window_to_workspace() {
    let handle = run_compositor_test(default_config(), 0, |data, phase| match phase {
        0 if active_workspace_has_n_windows(data, 1) => {
            assert_window_focused_in_workspace(data, WorkspaceName::Id(1));

            data.compositor
                .move_window_to_workspace(WorkspaceName::Id(2), true);
            TestState::Running(1)
        }
        1 => {
            assert_active_workspace_name(data, WorkspaceName::Id(2));
            assert!(workspace_has_n_windows(data, WorkspaceName::Id(1), 0));
            assert!(workspace_has_n_windows(data, WorkspaceName::Id(2), 1));
            assert_window_focused_in_workspace(data, WorkspaceName::Id(2));

            data.compositor
                .move_window_to_workspace(WorkspaceName::Id(1), false);
            TestState::Running(2)
        }
        2 => done(|data| {
            assert_active_workspace_name(data, WorkspaceName::Id(2));
            assert!(workspace_has_n_windows(data, WorkspaceName::Id(1), 1));
            assert!(workspace_has_n_windows(data, WorkspaceName::Id(2), 0));
            let window = get_next_window_in_workspace(data, WorkspaceName::Id(1));
            assert!(!is_focused(data, window.wl_surface()));
        }),
        _ => TestState::Running(phase),
    });

    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn test_focus_window_in_direction() {
    let handle = run_compositor_test(
        tiling_config("nodes = [{ repeat = 2 }]"),
        0,
        |data, phase| match phase {
            0 if active_workspace_has_n_windows(data, 2) => {
                let mapped = get_tiling_window(data, 1);
                assert!(is_focused(data, mapped.wl_surface()));
                data.compositor
                    .focus_adjacent_tiling_window(Direction::LEFT);
                TestState::Running(1)
            }
            1 => done(|data| {
                let mapped = get_tiling_window(data, 0);
                assert!(is_focused(data, mapped.wl_surface()));
            }),
            _ => TestState::Running(phase),
        },
    );

    spawn_alacritty(None);
    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn test_swap_window_in_direction() {
    let titles = [0, 1];
    let handle = run_compositor_test(
        tiling_config(r#"nodes = [{ repeat = 2 }]"#),
        0,
        move |data, phase| match phase {
            0 if active_workspace_has_n_windows(data, 2) => {
                assert_tiling_windows_title(data, titles.iter().map(|n| n.to_string()).collect());
                let mapped = get_tiling_window(data, 1);
                assert!(is_focused(data, mapped.wl_surface()));
                data.compositor
                    .swap_with_adjacent_tiling_window(Direction::LEFT);
                TestState::Running(1)
            }
            1 => {
                assert_tiling_windows_title(
                    data,
                    titles.iter().rev().map(|n| n.to_string()).collect(),
                );
                TestState::Running(2)
            }
            2 => done(|_| {}),
            _ => TestState::Running(phase),
        },
    );

    for title in titles {
        spawn_alacritty(Some(title.to_string()));
    }

    handle.join().unwrap();
}

#[test]
fn test_resize_window_in_direction() {
    let handle = run_compositor_test(
        tiling_config(r#"nodes = [{ repeat = 2 }]"#),
        0,
        move |data, phase| match phase {
            0 if active_workspace_has_n_windows(data, 2) => {
                let mapped = get_tiling_window(data, 0);
                assert_eq!(mapped.get_size(), (50, 100).into());
                let mapped = get_tiling_window(data, 1);
                assert_eq!(mapped.get_size(), (50, 100).into());
                data.compositor
                    .resize_adjacent_tiling_window(Direction::LEFT, WindowUnit::Px(10));
                TestState::Running(1)
            }
            1 => done(|data| {
                let mapped = get_tiling_window(data, 0);
                assert_eq!(mapped.get_size(), (40, 100).into());
                let mapped = get_tiling_window(data, 1);
                assert_eq!(mapped.get_size(), (60, 100).into());
            }),
            _ => TestState::Running(phase),
        },
    );

    spawn_alacritty(None);
    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn test_toggle_window_floating() {
    let handle =
        run_compositor_test(
            tiling_config(r#"nodes = [{ }]"#),
            0,
            move |data, phase| match phase {
                0 if active_workspace_has_n_windows(data, 1) => {
                    assert_eq!(get_floating_windows_count(data), 0);
                    assert_eq!(get_tiling_windows_count(data), 1);
                    data.compositor.toggle_window_floating_state(None);
                    TestState::Running(1)
                }
                1 => done(|data| {
                    assert_eq!(get_floating_windows_count(data), 1);
                    assert_eq!(get_tiling_windows_count(data), 0);
                }),
                _ => TestState::Running(phase),
            },
        );

    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn test_toggle_floating_window_hidden() {
    let elems = RefCell::new(0);
    let handle =
        run_compositor_test(
            tiling_config(r#"nodes = [{ }]"#),
            0,
            move |data, phase| match phase {
                0 if active_workspace_has_n_windows(data, 2) => {
                    let monitor = data.compositor.monitors.get_monitor_mut();
                    let workspace =
                        monitor.get_workspace_mut(&monitor.get_active_workspace_name().clone());
                    let renderer = data.backend.headless().get_renderer();
                    *elems.borrow_mut() = workspace.render_elements(renderer).len();

                    assert!(!workspace.is_floating_hidden());
                    let mapped = get_floating_window(data);
                    assert!(is_focused(data, mapped.wl_surface()));

                    data.compositor
                        .set_floating_window_visibility(None, Some(true));
                    TestState::Running(1)
                }
                1 => {
                    let monitor = data.compositor.monitors.get_monitor_mut();
                    let workspace =
                        monitor.get_workspace_mut(&monitor.get_active_workspace_name().clone());
                    let renderer = data.backend.headless().get_renderer();
                    assert!(*elems.borrow() > workspace.render_elements(renderer).len());

                    assert!(workspace.is_floating_hidden());
                    let mapped = get_tiling_window(data, 0);
                    assert!(is_focused(data, mapped.wl_surface()));

                    data.compositor
                        .set_floating_window_visibility(None, Some(true));
                    TestState::Running(2)
                }
                2 => {
                    let workspace = get_active_workspace(data);
                    assert!(!workspace.is_floating_hidden());
                    let mapped = get_floating_window(data);
                    assert!(is_focused(data, mapped.wl_surface()));

                    data.compositor
                        .set_floating_window_visibility(Some(true), Some(false));
                    TestState::Running(3)
                }
                3 => {
                    let _workspace = get_active_workspace(data);
                    let mapped = get_tiling_window(data, 0);
                    assert!(is_focused(data, mapped.wl_surface()));

                    data.compositor
                        .set_floating_window_visibility(Some(false), Some(false));
                    TestState::Running(4)
                }
                4 => done(|data| {
                    let _workspace = get_active_workspace(data);
                    let mapped = get_tiling_window(data, 0);
                    assert!(is_focused(data, mapped.wl_surface()));
                }),
                _ => TestState::Running(phase),
            },
        );

    spawn_alacritty(None);
    spawn_alacritty(None);

    handle.join().unwrap();
}
