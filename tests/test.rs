use std::{
    process::{Command, Stdio},
    thread::{sleep, spawn},
    time::Duration,
};

use loomwm::config::Config;

use crate::common::{assert_tiling_windows_title, has_n_windows, run_compositor_test};

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
    let h = run_compositor_test(default_config(), |d| has_n_windows(d, 1), |_| {});

    spawn(move || {
        let _ = Command::new("alacritty").stderr(Stdio::null()).spawn();
    });

    h.join().unwrap();
}

#[test]
fn todo_2() {
    let ts = [0, 1, 2];
    let ts_l = ts.len();
    let h = run_compositor_test(
        tiling_config("nodes = [{ repeat = 3 }]"),
        move |d| has_n_windows(d, ts_l),
        move |d| {
            assert_tiling_windows_title(d, ts.iter().map(|n| n.to_string()).collect());
        },
    );

    for i in 0..ts_l {
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
    let h = run_compositor_test(
        tiling_config("nodes = [{ }]"),
        |d| has_n_windows(d, 2),
        |d| {
            let m = d.compositor.monitors.get_monitor();
            let w = m.get_workspace(m.get_active_workspace_name());
            assert!(w.floating_windows_iter().count() == 1);
            assert!(w.tiling_windows_iter().count() == 1);
        },
    );

    for _ in 0..2 {
        spawn(move || {
            let _ = Command::new("alacritty").stderr(Stdio::null()).spawn();
        });
    }

    h.join().unwrap();
}

#[test]
fn todo_4() {
    let h = run_compositor_test(
        default_config(),
        |d| has_n_windows(d, 1),
        |d| {
            let m = d.compositor.monitors.get_monitor();
            let w = m.get_workspace(m.get_active_workspace_name());
            let m_w = w.floating_windows_iter().next().unwrap();
            assert!(m_w.get_focus());
            assert!(
                d.compositor
                    .get_keyboard()
                    .current_focus()
                    .is_some_and(|s| m_w.wl_surface() == s)
            );
        },
    );

    spawn(move || {
        let _ = Command::new("alacritty").stderr(Stdio::null()).spawn();
    });

    h.join().unwrap();
}
