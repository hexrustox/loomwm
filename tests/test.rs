use std::{
    process::{Command, Stdio},
    thread::{sleep, spawn},
    time::Duration,
};

use loomwm::{config::Config, utils::get_app_id_and_title};

use crate::common::{TestState, assert_tiling_windows_title, has_n_windows, run_compositor_test};

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
fn todo_1() {
    let h = run_compositor_test(default_config(), (), |d, _| {
        if has_n_windows(d, 1) {
            TestState::Done(Box::new(|_| {}))
        } else {
            TestState::Running(())
        }
    });

    spawn(move || {
        let _ = Command::new("alacritty").stderr(Stdio::null()).spawn();
    });

    h.join().unwrap();
}

#[test]
fn todo_2() {
    let ts = [0, 1, 2];

    let h = run_compositor_test(
        tiling_config("nodes = [{ repeat = 3 }]"),
        (),
        move |d, _| {
            if has_n_windows(d, 3) {
                TestState::Done(Box::new(move |d| {
                    assert_tiling_windows_title(d, ts.iter().map(|n| n.to_string()).collect());
                }))
            } else {
                TestState::Running(())
            }
        },
    );

    for i in ts {
        spawn(move || {
            let _ = Command::new("alacritty")
                .args(["-T", &i.to_string()])
                .stderr(Stdio::null())
                .spawn();
        });
        sleep(Duration::from_millis(50));
    }

    h.join().unwrap();
}

#[test]
fn todo_3() {
    let h = run_compositor_test(tiling_config("nodes = [{ }]"), (), |d, _| {
        if has_n_windows(d, 2) {
            TestState::Done(Box::new(|d| {
                let m = d.compositor.monitors.get_monitor();
                let w = m.get_workspace(m.get_active_workspace_name());
                assert!(w.floating_windows_iter().count() == 1);
                assert!(w.tiling_windows_iter().count() == 1);
            }))
        } else {
            TestState::Running(())
        }
    });

    for _ in 0..2 {
        spawn(move || {
            let _ = Command::new("alacritty").stderr(Stdio::null()).spawn();
        });
    }

    h.join().unwrap();
}

#[test]
fn todo_4() {
    let h = run_compositor_test(default_config(), (), |d, _| {
        if has_n_windows(d, 1) {
            TestState::Done(Box::new(|d| {
                let m = d.compositor.monitors.get_monitor();
                let w = m.get_workspace(m.get_active_workspace_name());
                let m_w = w.floating_windows_iter().next().unwrap();
                assert!(
                    d.compositor
                        .get_keyboard()
                        .current_focus()
                        .is_some_and(|s| m_w.wl_surface() == s)
                );
            }))
        } else {
            TestState::Running(())
        }
    });

    spawn(move || {
        let _ = Command::new("alacritty").stderr(Stdio::null()).spawn();
    });

    h.join().unwrap();
}

#[test]
fn todo_5() {
    let ts = [0, 1];

    let h = run_compositor_test(default_config(), 0u8, move |d, phase| match phase {
        0 if has_n_windows(d, 2) => {
            assert_tiling_windows_title(d, ts.iter().map(|n| n.to_string()).collect());
            let m = d.compositor.monitors.get_monitor();
            let w = m.get_workspace(m.get_active_workspace_name());
            let mut iter = w.floating_windows_iter();
            let m_w = iter.next().unwrap();
            assert!(
                d.compositor
                    .get_keyboard()
                    .current_focus()
                    .is_some_and(|s| m_w.wl_surface() == s)
            );
            let s = m_w.wl_surface();
            let (_, t) = get_app_id_and_title(&s);
            assert_eq!(t, "1");
            drop(iter);

            d.compositor.close_focused_window();
            TestState::Running(1)
        }
        1 if has_n_windows(d, 1) => TestState::Done(Box::new(|d| {
            let m = d.compositor.monitors.get_monitor();
            let w = m.get_workspace(m.get_active_workspace_name());
            let m_w = w.floating_windows_iter().next().unwrap();
            let (_, t) = get_app_id_and_title(&m_w.wl_surface());
            assert_eq!(t, "0");
            assert!(
                d.compositor
                    .get_keyboard()
                    .current_focus()
                    .is_some_and(|s| m_w.wl_surface() == s)
            );
        })),
        _ => TestState::Running(phase),
    });

    for i in ts {
        spawn(move || {
            let _ = Command::new("alacritty")
                .args(["-T", &i.to_string()])
                .stderr(Stdio::null())
                .spawn();
        });
        sleep(Duration::from_millis(50));
    }

    h.join().unwrap();
}
