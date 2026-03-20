use std::{
    process::{Command, Stdio},
    thread::{sleep, spawn},
    time::Duration,
};

use loomwm::config::Config;

use crate::common::{assert_tiling_windows_title, assert_window_count, run_compositor_test};

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
    let h = run_compositor_test(default_config(), |d| {
        assert_window_count(d, 1);
        true
    });

    spawn(move || {
        let _ = Command::new("alacritty").stderr(Stdio::null()).spawn();
    });

    h.join().unwrap();
}

#[test]
fn todo_2() {
    let h = run_compositor_test(tiling_config("nodes = [{ repeat = 3 }]"), |d| {
        assert_tiling_windows_title(
            d,
            vec![0, 1, 2].into_iter().map(|n| n.to_string()).collect(),
        );
        true
    });

    for i in 0..3 {
        spawn(move || {
            let _ = Command::new("alacritty")
                .args(["-T", &i.to_string()])
                .stderr(Stdio::null())
                .spawn();
        });
        sleep(Duration::from_millis(100));
    }

    h.join().unwrap();
}

#[test]
fn todo_3() {
    let h = run_compositor_test(tiling_config("nodes = [{ }]"), |d| {
        let m = d.compositor.monitors.get_monitor();
        let w = m.get_workspace(m.get_active_workspace_name());
        w.floating_windows_iter().count() == 1 && w.tiling_windows_iter().count() == 1
    });

    for _ in 0..2 {
        spawn(move || {
            let _ = Command::new("alacritty").stderr(Stdio::null()).spawn();
        });
    }

    h.join().unwrap();
}
