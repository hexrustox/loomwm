use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::LoopHandle,
    utils::Transform,
};

use crate::{CompositorData, state::WindowManagerState, utils::get_monotonic_time};

pub struct Winit {
    output: Output,
    backend: WinitGraphicsBackend<GlesRenderer>,
    damage_tracker: OutputDamageTracker,
}

impl Winit {
    pub fn new(event_loop: LoopHandle<CompositorData>) -> Result<Self, Box<dyn std::error::Error>> {
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

        event_loop.insert_source(winit, |event, _, data| {
            use WinitEvent::*;
            match event {
                Resized { size, .. } => {
                    data.backend.winit().unwrap().output.change_current_state(
                        Some(Mode { size, refresh: 60 }),
                        None,
                        None,
                        None,
                    );
                }
                Input(event) => data.compositor.process_input_event(event),
                Redraw => {
                    // TODO redraw queue
                    let winit = &mut data.backend.winit().unwrap();
                    let output = &winit.output;
                    let backend = &mut winit.backend;

                    let result = {
                        let (renderer, mut framebuffer) = backend.bind().unwrap();

                        let scale = output.current_scale().fractional_scale().into();
                        let elements = data.compositor.render_elements(renderer, scale);

                        winit.damage_tracker.render_output(
                            renderer,
                            &mut framebuffer,
                            0,
                            &elements,
                            [0.0, 0.0, 0.0, 1.0],
                        )
                    };

                    if let Ok(render_output) = result {
                        backend
                            .submit(render_output.damage.map(|damage| &**damage))
                            .unwrap();
                        data.compositor
                            .windows_in_active_workspace_iter()
                            .for_each(|mapped| {
                                mapped.window().send_frame(
                                    output,
                                    get_monotonic_time(),
                                    None,
                                    |_, _| Some(output.clone()),
                                );
                            });
                    }

                    data.compositor.space.refresh();
                    data.compositor.popups.cleanup();
                    let _ = data.compositor.display_handle.flush_clients();

                    backend.window().request_redraw();
                }
                Focus(_) => {}
                CloseRequested => {
                    data.compositor.event_signal.stop();
                }
            }
        })?;

        Ok(Self {
            output,
            backend,
            damage_tracker,
        })
    }

    pub fn init(&mut self, data: &mut WindowManagerState) {
        self.output
            .create_global::<WindowManagerState>(&data.display_handle);
        data.space.map_output(&self.output, (0, 0));
        data.monitors.push(
            self.output.clone(),
            data.layout_set.clone(),
            data.default_layout.clone(),
        );
    }
}
