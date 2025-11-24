use smithay::{
    backend::{
        renderer::{
            damage::OutputDamageTracker, element::surface::WaylandSurfaceRenderElement,
            gles::GlesRenderer,
        },
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::LoopHandle,
};

use crate::state::WMState;

pub struct Winit {
    output: Output,
    backend: WinitGraphicsBackend<GlesRenderer>,
    damage_tracker: OutputDamageTracker,
}

impl Winit {
    pub fn new(event_loop: LoopHandle<WMState>) -> Result<Self, Box<dyn std::error::Error>> {
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
        output.change_current_state(Some(mode), None, None, None);
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
                Input(event) => {}
                Redraw => {
                    let winit = &mut state.backend.winit().unwrap();
                    let age = winit.backend.buffer_age().unwrap_or(0);
                    let backend = &mut winit.backend;
                    let result = {
                        let (renderer, mut framebuffer) = backend.bind().unwrap();
                        winit
                            .damage_tracker
                            .render_output::<WaylandSurfaceRenderElement<_>, _>(
                                renderer,
                                &mut framebuffer,
                                age,
                                &[],
                                [0.1, 0.1, 0.1, 1.0],
                            )
                    };

                    if let Ok(render_output) = result {
                        backend.submit(render_output.damage.map(|v| &**v)).unwrap();
                    }

                    state.popups.cleanup();
                    let _ = state.display_handle.flush_clients();

                    backend.window().request_redraw();
                }
                Focus(_) => {}
                CloseRequested => {
                    state.event_signal.stop();
                }
            }
        })?;

        Ok(Self {
            output,
            backend,
            damage_tracker,
        })
    }
}
