use evdev::KeyCode;
use loomwm::{
    config::Config,
    input::grabs::{
        floating_resize_grab::FloatingResizeGrab, move_grab::MoveGrab, swap_grab::SwapGrab,
        tiling_resize_grab::TilingResizeGrab,
    },
    monitor::TileTreeWindow,
    utils::Direction,
};
use smithay::{
    backend::input::ButtonState,
    input::pointer::{ButtonEvent, Focus, GrabStartData as PointerGrabStartData, MotionEvent},
    utils::{Logical, Point, Rectangle, SERIAL_COUNTER, Size},
};

use crate::integration_tests::common::{
    TestState, active_workspace_has_n_windows, assert_tiling_windows_title, default_config, done,
    get_active_workspace_name, get_floating_window, get_tiling_window, run_compositor_test,
    spawn_alacritty, tiling_config,
};

use test_case::test_case;

#[test_case((5., 10.).into(); "move_down_right")]
#[test_case((-10., -5.).into(); "move_up_left")]
fn test_move_grab(delta: Point<f64, Logical>) {
    let handle = run_compositor_test(default_config(), 0, move |data, phase| match phase {
        0 if active_workspace_has_n_windows(data, 1) => {
            let mapped = get_floating_window(data);
            assert_eq!(mapped.get_location(), (0, 0).into());

            let location = mapped.get_location().to_f64();
            let pointer = data.compositor.get_pointer();
            let serial = SERIAL_COUNTER.next_serial();

            let start_data = PointerGrabStartData {
                focus: None,
                button: KeyCode::BTN_LEFT.0 as u32,
                location,
            };
            let grab = MoveGrab::new(start_data, mapped, location);
            pointer.set_grab(&mut data.compositor, grab, serial, Focus::Clear);

            let motion_location = (location.x + delta.x, location.y + delta.y).into();
            pointer.motion(
                &mut data.compositor,
                None,
                &MotionEvent {
                    location: motion_location,
                    serial: SERIAL_COUNTER.next_serial(),
                    time: 0,
                },
            );

            TestState::Running(1)
        }
        1 => done(move |data| {
            let mapped = get_floating_window(data);
            assert_eq!(mapped.get_location(), delta.to_i32_round());
        }),
        _ => TestState::Running(phase),
    });

    spawn_alacritty(None);

    handle.join().unwrap();
}

fn window_size_config(width: i32, height: i32) -> Config {
    toml::from_str(&format!(
        "[[window-rules]]
open-as-floating = {{ size = [{}, {}] }}
",
        width, height
    ))
    .unwrap()
}

#[test_case(
        Direction::RIGHT,
        (5., 5.).into(),
        (0, 0).into(),
        (55, 50).into();
        "resize_right"
    )]
#[test_case(
        Direction::BOTTOM,
        (5., 5.).into(),
        (0, 0).into(),
        (50, 55).into();
        "resize_bottom"
    )]
#[test_case(
        Direction::BOTTOM_RIGHT,
        (5., 5.).into(),
        (0, 0).into(),
        (55, 55).into();
        "resize_bottom_right"
    )]
#[test_case(
        Direction::TOP_LEFT,
        (5., 5.).into(),
        (5, 5).into(),
        (45, 45).into();
        "resize_top_left"
    )]
fn test_floating_resize_grab(
    direction: Direction,
    delta: Point<f64, Logical>,
    expected_loc: Point<i32, Logical>,
    expected_size: Size<i32, Logical>,
) {
    let handle = run_compositor_test(
        window_size_config(50, 50),
        0,
        move |data, phase| match phase {
            0 if active_workspace_has_n_windows(data, 1) => {
                let mapped = get_floating_window(data);
                let size = mapped.get_size();
                let location = mapped.get_location();
                assert_eq!(size, (50, 50).into());
                assert_eq!(location, (0, 0).into());

                let pointer = data.compositor.get_pointer();
                let pointer_location = pointer.current_location();
                let serial = SERIAL_COUNTER.next_serial();

                let start_data = PointerGrabStartData {
                    focus: None,
                    button: KeyCode::BTN_LEFT.0 as u32,
                    location: pointer_location,
                };
                let initial_rect = Rectangle::new(location, size);
                let grab = FloatingResizeGrab::new(start_data, mapped, direction, initial_rect);
                pointer.set_grab(&mut data.compositor, grab, serial, Focus::Clear);

                let motion_location = pointer_location + delta;
                pointer.motion(
                    &mut data.compositor,
                    None,
                    &MotionEvent {
                        location: motion_location,
                        serial: SERIAL_COUNTER.next_serial(),
                        time: 0,
                    },
                );
                pointer.button(
                    &mut data.compositor,
                    &ButtonEvent {
                        button: KeyCode::BTN_LEFT.0 as u32,
                        state: ButtonState::Released,
                        serial: SERIAL_COUNTER.next_serial(),
                        time: 0,
                    },
                );

                TestState::Running(1)
            }
            1 => done(move |data| {
                let mapped = get_floating_window(data);
                assert_eq!(mapped.get_location(), expected_loc);
                assert_eq!(mapped.get_size(), expected_size);
            }),
            _ => TestState::Running(phase),
        },
    );

    spawn_alacritty(None);

    handle.join().unwrap();
}

#[test]
fn test_swap_grab() {
    let titles = [0, 1];
    let handle = run_compositor_test(
        tiling_config(r#"nodes = [{ repeat = 2 }]"#),
        0,
        move |data, phase| match phase {
            0 if active_workspace_has_n_windows(data, 2) => {
                let mapped = get_tiling_window(data, 0);
                let workspace_name = get_active_workspace_name(data).clone();
                let hidden = data
                    .compositor
                    .get_focused_workspace_floating_window_hidden();

                let pointer = data.compositor.get_pointer();
                let pointer_location = pointer.current_location();
                let serial = SERIAL_COUNTER.next_serial();

                let start_data = PointerGrabStartData {
                    focus: None,
                    button: KeyCode::BTN_LEFT.0 as u32,
                    location: pointer_location,
                };
                let grab = SwapGrab::new(start_data, mapped, workspace_name, hidden);
                pointer.set_grab(&mut data.compositor, grab, serial, Focus::Clear);

                let motion_location = get_tiling_window(data, 1).center_location().to_f64();
                pointer.motion(
                    &mut data.compositor,
                    None,
                    &MotionEvent {
                        location: motion_location,
                        serial: SERIAL_COUNTER.next_serial(),
                        time: 0,
                    },
                );

                pointer.button(
                    &mut data.compositor,
                    &ButtonEvent {
                        button: KeyCode::BTN_LEFT.0 as u32,
                        state: smithay::backend::input::ButtonState::Released,
                        serial: SERIAL_COUNTER.next_serial(),
                        time: 0,
                    },
                );

                TestState::Running(1)
            }
            1 => done(move |data| {
                assert_tiling_windows_title(
                    data,
                    titles.iter().rev().map(|n| n.to_string()).collect(),
                );
            }),
            _ => TestState::Running(phase),
        },
    );

    for title in titles {
        spawn_alacritty(Some(title.to_string()));
    }

    handle.join().unwrap();
}

#[test]
fn test_tiling_resize_grab() {
    let handle = run_compositor_test(
        toml::from_str(
            r#"[layouts]
main = { nodes = [{ layout = "sub", repeat = 2 }] }
sub = { split = "horizontal", nodes = [{ repeat = 2 }] }
default = "main"
"#,
        )
        .unwrap(),
        0,
        |data, phase| match phase {
            0 if active_workspace_has_n_windows(data, 4) => {
                let mapped = get_tiling_window(data, 3);
                let initial_size = mapped.get_size();
                assert_eq!(mapped.get_location(), (50, 50).into());
                assert_eq!(initial_size, (50, 50).into());

                let pointer = data.compositor.get_pointer();
                let pointer_location = pointer.current_location();
                let serial = SERIAL_COUNTER.next_serial();
                let start_data = PointerGrabStartData {
                    focus: None,
                    button: KeyCode::BTN_LEFT.0 as u32,
                    location: pointer_location,
                };
                let grab = TilingResizeGrab::new(start_data, Direction::TOP_LEFT, initial_size);
                pointer.set_grab(&mut data.compositor, grab, serial, Focus::Clear);

                let motion_location = (pointer_location.x - 5., pointer_location.y - 5.).into();
                pointer.motion(
                    &mut data.compositor,
                    None,
                    &MotionEvent {
                        location: motion_location,
                        serial: SERIAL_COUNTER.next_serial(),
                        time: 0,
                    },
                );
                TestState::Running(1)
            }
            1 => done(|data| {
                let mapped = get_tiling_window(data, 0);
                assert_eq!(mapped.get_location(), (0, 0).into());
                assert_eq!(mapped.get_size(), (45, 50).into());
                let mapped = get_tiling_window(data, 1);
                assert_eq!(mapped.get_location(), (0, 50).into());
                assert_eq!(mapped.get_size(), (45, 50).into());
                let mapped = get_tiling_window(data, 2);
                assert_eq!(mapped.get_location(), (45, 0).into());
                assert_eq!(mapped.get_size(), (55, 45).into());
                let mapped = get_tiling_window(data, 3);
                assert_eq!(mapped.get_location(), (45, 45).into());
                assert_eq!(mapped.get_size(), (55, 55).into());
            }),
            _ => TestState::Running(phase),
        },
    );
    for i in 0..4 {
        spawn_alacritty(Some(i.to_string()));
    }
    handle.join().unwrap();
}
