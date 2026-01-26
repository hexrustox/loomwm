use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::LoopHandle,
    utils::Transform,
};

use crate::{AppState, state::WaylandState, utils::get_monotonic_time};

pub struct Winit {
    output: Output,
    backend: WinitGraphicsBackend<GlesRenderer>,
    damage_tracker: OutputDamageTracker,
}

impl Winit {
    pub fn new(event_loop: LoopHandle<AppState>) -> Result<Self, Box<dyn std::error::Error>> {
        let (backend, winit) = winit::init()?;

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

        event_loop.insert_source(winit, |event, _, state| {
            use WinitEvent::*;
            match event {
                Resized { size, .. } => {
                    state.backend.winit().unwrap().output.change_current_state(
                        Some(Mode { size, refresh: 60 }),
                        None,
                        None,
                        None,
                    );
                }
                Input(event) => state.compositor.process_input_event(event),
                Redraw => {
                    let winit = &mut state.backend.winit().unwrap();
                    let output = &winit.output;
                    let backend = &mut winit.backend;

                    let result = {
                        let age = backend.buffer_age().unwrap_or(0);
                        let (renderer, mut framebuffer) = backend.bind().unwrap();

                        let scale = output.current_scale().fractional_scale().into();
                        let elements = state.compositor.workspaces.get_active().render_elements(
                            &state.compositor.windows.mapped_windows,
                            renderer,
                            scale,
                        );

                        winit.damage_tracker.render_output(
                            renderer,
                            &mut framebuffer,
                            age,
                            &elements,
                            [0.1, 0.1, 0.1, 1.0],
                        )
                    };

                    if let Ok(render_output) = result {
                        backend.submit(render_output.damage.map(|v| &**v)).unwrap();
                        state
                            .compositor
                            .windows
                            .mapped_windows
                            .values()
                            .for_each(|w| {
                                w.inner
                                    .send_frame(output, get_monotonic_time(), None, |_, _| {
                                        Some(output.clone())
                                    });
                            });
                    }

                    state.compositor.popups.cleanup();
                    let _ = state.compositor.display_handle.flush_clients();

                    backend.window().request_redraw();
                }
                Focus(_) => {}
                CloseRequested => {
                    state.compositor.event_signal.stop();
                }
            }
        })?;

        Ok(Self {
            output,
            backend,
            damage_tracker,
        })
    }

    pub fn init(&mut self, compositor: &mut WaylandState) {
        compositor.space.map_output(&self.output, (0, 0));
    }
}
