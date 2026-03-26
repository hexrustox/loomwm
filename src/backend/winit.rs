use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::LoopHandle,
    utils::Transform,
};

use crate::{CompositorData, state::WindowManagerState};

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
        output.change_current_state(
            Some(mode),
            Some(Transform::Flipped180),
            Some(smithay::output::Scale::Fractional(1.0)),
            None,
        );
        output.set_preferred(mode);

        let damage_tracker = OutputDamageTracker::from_output(&output);

        event_loop.insert_source(winit, |event, _, data| {
            use WinitEvent::*;
            match event {
                Resized { size, .. } => {
                    // FIXME
                    data.backend.winit().output.change_current_state(
                        Some(Mode { size, refresh: 60 }),
                        None,
                        None,
                        None,
                    );
                    data.compositor.update_workspaces_tiling_windows_size();
                }
                Input(event) => data.compositor.process_input_event(event),
                // Redraw => {}
                CloseRequested => {
                    data.compositor.event_signal.stop();
                }
                _ => {}
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
            &data.default_layout,
        );
    }

    pub fn render(&mut self, data: &mut WindowManagerState) {
        let elements = data.render_elements(self.backend.renderer());

        let result = {
            let (renderer, mut framebuffer) = self.backend.bind().unwrap();

            self.damage_tracker
                .render_output(renderer, &mut framebuffer, 0, &elements, [0.; 4])
                .unwrap()
        };

        if let Some(damage) = result.damage {
            self.backend.submit(Some(damage)).unwrap();
        }

        self.backend.window().request_redraw();
    }
}
