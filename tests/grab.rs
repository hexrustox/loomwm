#[allow(dead_code)]
mod common;

mod integration_tests {
    use evdev::KeyCode;
    use loomwm::{
        config::Config,
        input::grabs::{floating_resize_grab::FloatingResizeGrab, move_grab::MoveGrab},
        monitor::TileTreeWindow,
        utils::Direction,
    };
    use smithay::{
        input::pointer::{Focus, GrabStartData as PointerGrabStartData, MotionEvent},
        utils::{Logical, Point, Rectangle, SERIAL_COUNTER, Size},
    };

    use crate::common::{
        TestState, active_workspace_has_n_windows, default_config, done, get_floating_window,
        run_compositor_test, spawn_alacritty,
    };

    #[test]
    fn test_move_grab() {
        let handle = run_compositor_test(default_config(), 0, |data, phase| match phase {
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

                let motion_location = (location.x + 5., location.y + 5.).into();
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
                let mapped = get_floating_window(data);
                assert_eq!(mapped.get_location(), (5, 5).into());
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

    fn test_floating_resize_grab_internal(
        direction: Direction,
        delta: Point<f64, Logical>,
        expected_loc: Point<i32, Logical>,
        expected_size: Size<i32, Logical>,
    ) {
        let handle =
            run_compositor_test(
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
                        let grab =
                            FloatingResizeGrab::new(start_data, mapped, direction, initial_rect);
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
    fn test_floating_resize_grab_bottom_right() {
        test_floating_resize_grab_internal(
            Direction::BOTTOM_RIGHT,
            (5., 5.).into(),
            (0, 0).into(),
            (55, 55).into(),
        );
    }

    #[test]
    fn test_floating_resize_grab_top_left() {
        test_floating_resize_grab_internal(
            Direction::TOP_LEFT,
            (5., 5.).into(),
            (5, 5).into(),
            (45, 45).into(),
        );
    }
}
