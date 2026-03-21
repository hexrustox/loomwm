use loomwm::{
    config::Config,
    monitor::WorkspaceName,
    utils::{Direction, get_app_id_and_title},
};

use crate::common::{
    TestState, active_workspace_has_n_windows, assert_tiling_windows_title, done,
    get_active_workspace, get_next_window_in_workspace, is_focused, run_compositor_test,
    spawn_alacritty, wait_until, workspace_has_n_windows,
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

mod integration_tests {
    use std::cell::RefCell;

    use loomwm::{input::WindowUnit, monitor::TileTreeWindow};

    use super::*;

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
                |data| active_workspace_has_n_windows(data, 2),
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
                |data| active_workspace_has_n_windows(data, 3),
                Box::new(move |data| {
                    assert_tiling_windows_title(
                        data,
                        titles.iter().map(|n| n.to_string()).collect(),
                    );
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
                let workspace = get_active_workspace(data);
                let mapped = workspace.windows_iter().next().unwrap();
                assert!(is_focused(data, mapped.wl_surface()));
                let (_, title) = get_app_id_and_title(&mapped.wl_surface());
                assert_eq!(title, "1");

                data.compositor.close_focused_window();
                TestState::Running(1)
            }
            1 if active_workspace_has_n_windows(data, 1) => done(|data| {
                let workspace = get_active_workspace(data);
                let mapped = workspace.windows_iter().next().unwrap();
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

    #[test]
    fn test_workspace_switch() {
        let handle = run_compositor_test(default_config(), 0, |data, phase| match phase {
            0 if active_workspace_has_n_windows(data, 1) => {
                data.compositor
                    .switch_or_create_active_workspace(WorkspaceName::Id(2));
                TestState::Running(1)
            }
            1 => {
                assert_eq!(
                    data.compositor
                        .monitors
                        .get_monitor()
                        .get_active_workspace_name(),
                    &WorkspaceName::Id(2)
                );
                spawn_alacritty(None);
                TestState::Running(2)
            }
            2 if active_workspace_has_n_windows(data, 1) => {
                let mapped = get_next_window_in_workspace(data, WorkspaceName::Id(2));
                assert!(is_focused(data, mapped.wl_surface()));
                data.compositor
                    .switch_or_create_active_workspace(WorkspaceName::Id(1));
                TestState::Running(3)
            }
            3 => {
                let mapped = get_next_window_in_workspace(data, WorkspaceName::Id(1));
                assert!(is_focused(data, mapped.wl_surface()));
                data.compositor
                    .switch_or_create_active_workspace(WorkspaceName::Id(2));
                TestState::Running(4)
            }
            4 => done(|data| {
                let mapped = get_next_window_in_workspace(data, WorkspaceName::Id(2));
                assert!(is_focused(data, mapped.wl_surface()));
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
                let mapped = get_next_window_in_workspace(data, WorkspaceName::Id(1));
                assert!(is_focused(data, mapped.wl_surface()));

                data.compositor
                    .move_focused_window_to_workspace(WorkspaceName::Id(2), true);
                TestState::Running(1)
            }
            1 => {
                assert_eq!(
                    data.compositor
                        .monitors
                        .get_monitor()
                        .get_active_workspace_name(),
                    &WorkspaceName::Id(2)
                );
                assert!(workspace_has_n_windows(data, WorkspaceName::Id(1), 0));
                assert!(workspace_has_n_windows(data, WorkspaceName::Id(2), 1));
                let mapped = get_next_window_in_workspace(data, WorkspaceName::Id(2));
                assert!(is_focused(data, mapped.wl_surface()));

                data.compositor
                    .move_focused_window_to_workspace(WorkspaceName::Id(1), false);
                TestState::Running(2)
            }
            2 => done(|data| {
                assert_eq!(
                    data.compositor
                        .monitors
                        .get_monitor()
                        .get_active_workspace_name(),
                    &WorkspaceName::Id(2)
                );
                assert!(workspace_has_n_windows(data, WorkspaceName::Id(1), 1));
                assert!(workspace_has_n_windows(data, WorkspaceName::Id(2), 0));
                let mapped = get_next_window_in_workspace(data, WorkspaceName::Id(1));
                assert!(!is_focused(data, mapped.wl_surface()));
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
                    {
                        let mut iter = get_active_workspace(data).tiling_windows_iter();
                        let _ = iter.next();
                        let mapped = iter.next().unwrap();
                        assert!(is_focused(data, mapped.wl_surface()));
                    }
                    data.compositor
                        .focus_tiling_window_in_direction(Direction::LEFT);
                    TestState::Running(1)
                }
                1 => done(|data| {
                    let mut iter = get_active_workspace(data).tiling_windows_iter();
                    let mapped = iter.next().unwrap();
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
                    assert_tiling_windows_title(
                        data,
                        titles.iter().map(|n| n.to_string()).collect(),
                    );
                    {
                        let mut iter = get_active_workspace(data).tiling_windows_iter();
                        let _ = iter.next();
                        let mapped = iter.next().unwrap();
                        assert!(is_focused(data, mapped.wl_surface()));
                    }
                    data.compositor
                        .swap_focused_tiling_window_in_direction(Direction::LEFT);
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
                    {
                        let mut iter = get_active_workspace(data).tiling_windows_iter();
                        let mapped = iter.next().unwrap();
                        assert_eq!(mapped.get_size(), (50, 100).into());
                        let mapped = iter.next().unwrap();
                        assert_eq!(mapped.get_size(), (50, 100).into());
                    }
                    data.compositor
                        .resize_focused_tiling_window(Direction::LEFT, WindowUnit::Px(10));
                    TestState::Running(1)
                }
                1 => done(|data| {
                    let mut iter = get_active_workspace(data).tiling_windows_iter();
                    let mapped = iter.next().unwrap();
                    assert_eq!(mapped.get_size(), (40, 100).into());
                    let mapped = iter.next().unwrap();
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
        let handle = run_compositor_test(
            tiling_config(r#"nodes = [{ }]"#),
            0,
            move |data, phase| match phase {
                0 if active_workspace_has_n_windows(data, 1) => {
                    let workspace = get_active_workspace(data);
                    assert_eq!(workspace.floating_windows_iter().count(), 0);
                    assert_eq!(workspace.tiling_windows_iter().count(), 1);
                    data.compositor.toggle_focused_window_floating(None);
                    TestState::Running(1)
                }
                1 => done(|data| {
                    let workspace = get_active_workspace(data);
                    assert_eq!(workspace.floating_windows_iter().count(), 1);
                    assert_eq!(workspace.tiling_windows_iter().count(), 0);
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
        let handle = run_compositor_test(
            tiling_config(r#"nodes = [{ }]"#),
            0,
            move |data, phase| match phase {
                0 if active_workspace_has_n_windows(data, 2) => {
                    let monitor = data.compositor.monitors.get_monitor_mut();
                    let workspace =
                        monitor.get_workspace_mut(&monitor.get_active_workspace_name().clone());
                    let renderer = data.backend.headless().get_renderer();
                    *elems.borrow_mut() = workspace.render_elements(renderer).len();

                    assert!(!workspace.get_floating_window_hidden());
                    let mapped = workspace.floating_windows_iter().next().unwrap().clone();
                    assert!(is_focused(data, mapped.wl_surface()));

                    data.compositor
                        .set_focused_workspace_floating_window_hidden(None, Some(true));
                    TestState::Running(1)
                }
                1 => {
                    let monitor = data.compositor.monitors.get_monitor_mut();
                    let workspace =
                        monitor.get_workspace_mut(&monitor.get_active_workspace_name().clone());
                    let renderer = data.backend.headless().get_renderer();
                    assert!(*elems.borrow() > workspace.render_elements(renderer).len());

                    assert!(workspace.get_floating_window_hidden());
                    let mapped = workspace.tiling_windows_iter().next().unwrap().clone();
                    assert!(is_focused(data, mapped.wl_surface()));

                    data.compositor
                        .set_focused_workspace_floating_window_hidden(None, Some(true));
                    TestState::Running(2)
                }
                2 => {
                    let workspace = get_active_workspace(data);
                    assert!(!workspace.get_floating_window_hidden());
                    let mapped = workspace.floating_windows_iter().next().unwrap().clone();
                    assert!(is_focused(data, mapped.wl_surface()));

                    data.compositor
                        .set_focused_workspace_floating_window_hidden(Some(true), Some(false));
                    TestState::Running(3)
                }
                3 => {
                    let workspace = get_active_workspace(data);
                    let mapped = workspace.tiling_windows_iter().next().unwrap().clone();
                    assert!(is_focused(data, mapped.wl_surface()));

                    data.compositor
                        .set_focused_workspace_floating_window_hidden(Some(false), Some(false));
                    TestState::Running(4)
                }
                4 => done(|data| {
                    let workspace = get_active_workspace(data);
                    let mapped = workspace.tiling_windows_iter().next().unwrap().clone();
                    assert!(is_focused(data, mapped.wl_surface()));
                }),
                _ => TestState::Running(phase),
            },
        );

        spawn_alacritty(None);
        spawn_alacritty(None);

        handle.join().unwrap();
    }
}
