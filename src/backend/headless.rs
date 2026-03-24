use smithay::{
    backend::{
        egl::{EGLContext, EGLDisplay, native::EGLSurfacelessDisplay},
        renderer::gles::GlesRenderer,
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    utils::{Physical, Size},
};

use crate::state::WindowManagerState;

pub struct Headless {
    renderer: Option<GlesRenderer>,
}

impl Default for Headless {
    fn default() -> Self {
        Self::new()
    }
}

impl Headless {
    pub fn new() -> Self {
        Self { renderer: None }
    }

    pub fn add_output(
        &mut self,
        data: &mut WindowManagerState,
        size: impl Into<Size<i32, Physical>>,
    ) {
        let output = Output::new(
            "headless".to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "Smithay".to_string(),
                model: "Headless".to_string(),
            },
        );

        let mode = Mode {
            size: size.into(),
            refresh: 60,
        };
        output.change_current_state(Some(mode), None, None, None);
        output.set_preferred(mode);

        data.space.map_output(&output, (0, 0));
        data.monitors
            .push(output, data.layout_set.clone(), &data.default_layout);
    }

    pub fn init(&mut self) {
        let renderer = unsafe {
            let display = EGLDisplay::new(EGLSurfacelessDisplay).unwrap();
            let context = EGLContext::new(&display).unwrap();
            GlesRenderer::new(context).unwrap()
        };

        self.renderer = Some(renderer);
    }

    pub fn get_renderer(&mut self) -> &mut GlesRenderer {
        self.renderer.as_mut().unwrap()
    }

    pub fn render(&mut self, data: &mut WindowManagerState) {
        let renderer = self.renderer.as_mut().unwrap();
        let _ = data.render_elements(renderer);
    }
}
