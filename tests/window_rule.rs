#[allow(dead_code)]
mod common;

mod integration_tests {
    use crate::common::{
        active_workspace_has_n_windows, get_active_workspace, get_floating_window, get_next_window_in_workspace, get_tiling_window, is_focused, run_compositor_test, spawn_alacritty, wait_until, workspace_has_n_windows
    };
    use loomwm::CompositorData;
    use loomwm::monitor::{TileTreeWindow, WorkspaceName};
    use loomwm::utils::RGBAColor;
    use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
    use test_case::test_case;

    #[test_case(
        r#""#,
        |data| {
            let mapped = get_floating_window(data);
            assert!(is_focused(data, mapped.wl_surface()));
        }; "focus_default"
    )]
    #[test_case(
        r#"[[window-rules]]
open-with-focus = false
"#,
        |data| {
            let mapped = get_floating_window(data);
            assert!(!is_focused(data, mapped.wl_surface()));
        }; "focus_false"
    )]
    #[test_case(
        r#"[[window-rules]]
open-as-floating = {}

[layouts]
main = { nodes = [{}] }
default = "main"
"#,
        |data| {
            let mapped = get_floating_window(data);
            assert!(mapped.get_floating());
        }; "state_float"
    )]
    #[test_case(
        r#"[[window-rules]]
open-in-workspace = 2
"#,
        |data| {
            let workspace = get_active_workspace(data);
            assert_eq!(workspace.get_name(), &WorkspaceName::Id(2));
            assert_eq!(workspace.windows_count(), 1);
        }; "workspace"
    )]
    #[test_case(
        r#"[[window-rules]]
decoration = "server_side"
"#,
        |data| {
            let mapped = get_floating_window(data);
            let toplevel = mapped.toplevel();
            let mode = toplevel.with_pending_state(|s| s.decoration_mode);
            assert_eq!(mode, Some(Mode::ServerSide));
        }; "decoration_server"
    )]
    #[test_case(
        r#"[[window-rules]]
decoration = "client_side"
"#,
        |data| {
            let mapped = get_floating_window(data);
            let toplevel = mapped.toplevel();
            let mode = toplevel.with_pending_state(|s| s.decoration_mode);
            assert_eq!(mode, Some(Mode::ClientSide));
        }; "decoration_client"
    )]
    #[test_case(
        r#"[[window-rules]]
border = { width = 2, color = "123456" }
"#,
        |data| {
            let mapped = get_floating_window(data);
            assert_eq!(mapped.get_border_width(), 2);
            assert_eq!(mapped.get_border_color(), Some(RGBAColor::new(0x123456ff)));
        }; "border"
    )]
    #[test_case(
        r#"[[window-rules]]
opacity = 0.5
"#,
        |data| {
            let mapped = get_floating_window(data);
            assert_eq!(mapped.get_opacity(), 0.5);
        }; "opacity"
    )]
    fn test_window_rule(config: &str, assertion: impl Fn(&mut CompositorData) + Send + 'static) {
        let handle = run_compositor_test(
            toml::from_str(config).unwrap(),
            (),
            wait_until(
                |data| active_workspace_has_n_windows(data, 1),
                Box::new(assertion),
            ),
        );

        spawn_alacritty(None);

        handle.join().unwrap();
    }

    #[test]
    fn test_window_rule_tiling() {
        let handle = run_compositor_test(
            toml::from_str(
                r#"[[window-rules]]
matches = [{ title = "0" }]
open-as-tiling = { ratio = 1.5 }

[layouts]
main = { nodes = [ { repeat = 2} ] }
default = "main""#,
            )
            .unwrap(),
            (),
            wait_until(
                |data| active_workspace_has_n_windows(data, 2),
                Box::new(|data| {
                    let mapped = get_tiling_window(data, 0);
                    assert_eq!(mapped.get_size(), (60, 100).into());
                }),
            ),
        );

        spawn_alacritty(Some("0".to_string()));
        spawn_alacritty(Some("1".to_string()));

        handle.join().unwrap();
    }

    #[test]
    fn test_window_rule_workspace_no_focus() {
        let handle = run_compositor_test(
            toml::from_str(
                r#"[[window-rules]]
matches = [{ title = "0" }]
open-with-focus = false
open-in-workspace = 2
"#,
            )
            .unwrap(),
            (),
            wait_until(
                |data| {
                    workspace_has_n_windows(data, WorkspaceName::Id(1), 1)
                        && workspace_has_n_windows(data, WorkspaceName::Id(2), 1)
                },
                Box::new(|data| {
                    let mapped = get_next_window_in_workspace(data, WorkspaceName::Id(1));
                    assert!(is_focused(data, mapped.wl_surface()));
                    let mapped = get_next_window_in_workspace(data, WorkspaceName::Id(2));
                    assert!(!is_focused(data, mapped.wl_surface()));
                }),
            ),
        );

        spawn_alacritty(Some("0".to_string()));
        spawn_alacritty(Some("1".to_string()));

        handle.join().unwrap();
    }
}
