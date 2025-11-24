use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::LoopHandle,
        winit::{dpi::LogicalSize, window::Window},
    },
    utils::{Scale, Transform},
};

use crate::{CallLoopData, Smallvil};

pub struct Winit {
    output: Output,
    backend: WinitGraphicsBackend<GlesRenderer>,
    damage_tracker: OutputDamageTracker,
}

impl Winit {
    pub fn new(event_loop: LoopHandle<CallLoopData>) -> Result<Self, winit::Error> {
        let builder = Window::default_attributes().with_inner_size(LogicalSize::new(1280.0, 800.0));
        let (backend, winit) = winit::init_from_attributes(builder)?;

        let output = Output::new(
            "winit".to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "Smithay".into(),
                model: "Winit".into(),
            },
        );

        let mode = Mode {
            size: backend.window_size(),
            refresh: 60_000,
        };
        output.change_current_state(Some(mode), Some(Transform::Flipped180), None, None);
        output.set_preferred(mode);

        let damage_tracker = OutputDamageTracker::from_output(&output);

        event_loop
            .insert_source(winit, move |event, _, data| match event {
                WinitEvent::Resized { size, .. } => {
                    let winit = &data.backend;
                    winit.output.change_current_state(
                        Some(Mode {
                            size,
                            refresh: 60_000,
                        }),
                        None,
                        None,
                        None,
                    );
                }
                WinitEvent::Input(event) => data.state.process_input_event(event),
                WinitEvent::Focus(_) => (),
                WinitEvent::Redraw => {
                    let display = &mut data.state.display_handle;
                    let backend = &mut data.backend.backend;
                    let output = &data.backend.output;

                    let res = {
                        let age = backend.buffer_age().unwrap_or_default();
                        let (renderer, mut framebuffer) = backend.bind().unwrap();
                        let output_scale = output.current_scale().fractional_scale();
                        let scale = Scale::from(output_scale);
                        let elements = data.state.layout.render_elements::<GlesRenderer>(
                            renderer,
                            scale,
                            data.state.start_time.elapsed(),
                        );
                        data.backend
                            .damage_tracker
                            .render_output(renderer, &mut framebuffer, age, &elements, [0.1; 4])
                            .unwrap()
                    };
                    if let Some(damage) = res.damage {
                        backend.submit(Some(damage)).unwrap();
                    }
                    data.state.space.refresh();
                    data.state.popups.cleanup();
                    let _ = display.flush_clients();
                    // Ask for redraw to schedule new frame.
                    backend.window().request_redraw();
                }
                WinitEvent::CloseRequested => data.state.loop_signal.stop(),
            })
            .unwrap();

        Ok(Self {
            output,
            backend,
            damage_tracker,
        })
    }

    pub fn init(&mut self, state: &mut Smallvil) {
        unsafe {
            std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);
        }

        state.add_output(self.output.clone());
    }
}
