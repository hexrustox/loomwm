use smithay::{
    backend::renderer::{
        ImportAll, Renderer, Texture,
        element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    },
    desktop::{Window, space::SpaceElement},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle, Scale},
};

#[derive(Clone)]
pub struct MyWindow {
    pub element: Window,
    location: Point<i32, Logical>,
}

impl MyWindow {
    fn geometry(&self) -> Rectangle<i32, Logical> {
        let mut geo = self.element.geometry();
        geo.loc = self.location;
        geo
    }

    fn bbox(&self) -> Rectangle<i32, Logical> {
        let mut bbox = self.element.bbox();
        bbox.loc += self.location - self.element.geometry().loc;
        bbox
    }

    fn render_location(&self) -> Point<i32, Logical> {
        self.location - self.element.geometry().loc
    }
}

pub struct MyWindowWrapper {
    elements: Vec<MyWindow>,
}

impl MyWindowWrapper {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    pub fn map_element<P>(&mut self, element: Window, location: P, activate: bool)
    where
        P: Into<Point<i32, Logical>>,
    {
        // let outputs =
        if let Some(pos) = self
            .elements
            .iter()
            .position(|inner| inner.element == element)
        {
            // self.elements.remove(pos).outputs
            self.elements.remove(pos);
        } else {
            // HashMap::new()
        };

        let inner = MyWindow {
            element,
            location: location.into(),
        };
        self.insert_elem(inner, activate);
    }

    pub fn raise_element(&mut self, element: &Window, activate: bool) {
        if let Some(pos) = self
            .elements
            .iter()
            .position(|inner| &inner.element == element)
        {
            let inner = self.elements.remove(pos);
            self.insert_elem(inner, activate);
        }
    }

    fn insert_elem(&mut self, elem: MyWindow, activate: bool) {
        if activate {
            elem.element.set_activate(true);
            for e in self.elements.iter() {
                e.element.set_activate(false);
            }
        }

        self.elements.push(elem);
        self.elements
            .sort_by(|e1, e2| e1.element.z_index().cmp(&e2.element.z_index()));
    }

    pub fn elements(&self) -> impl DoubleEndedIterator<Item = &Window> + ExactSizeIterator {
        self.elements.iter().map(|e| &e.element)
    }

    pub fn element_under<P: Into<Point<f64, Logical>>>(
        &self,
        point: P,
    ) -> Option<(&Window, Point<i32, Logical>)> {
        let point = point.into();
        self.elements
            .iter()
            .rev()
            .filter(|e| e.bbox().to_f64().contains(point))
            .find_map(|e| {
                // we need to offset the point to the location where the surface is actually drawn
                let render_location = e.render_location();
                if e.element
                    .is_in_input_region(&(point - render_location.to_f64()))
                {
                    Some((&e.element, render_location))
                } else {
                    None
                }
            })
    }

    pub fn element_location(&self, elem: &Window) -> Option<Point<i32, Logical>> {
        self.elements
            .iter()
            .find(|e| &e.element == elem)
            .map(|e| e.location)
    }

    pub fn element_geometry(&self, elem: &Window) -> Option<Rectangle<i32, Logical>> {
        self.elements
            .iter()
            .find(|e| &e.element == elem)
            .map(|e| e.geometry())
    }

    pub fn unmap_element(&mut self, look_for: LookForWindowBy) -> Option<MyWindow> {
        self.elements
            .iter()
            .position(|inner| match look_for {
                LookForWindowBy::Window(w) => inner.element == *w,
                LookForWindowBy::Surface(s) => inner.element.toplevel().unwrap().wl_surface() == s,
            })
            .map(|pos| {
                // let elem =
                self.elements.remove(pos)
                // for output in elem.outputs.keys() {
                //     elem.element.output_leave(output);
                // }
            })
    }

    pub fn set_element_layout(&mut self, layouts: &[Rectangle<i32, Logical>]) {
        let mut iter = layouts.iter();
        for w in &mut self.elements {
            let rect = iter.next().unwrap();
            w.location = rect.loc;
            let xdg = w.element.toplevel().unwrap();
            xdg.with_pending_state(|state| {
                state.size.replace(rect.size);
            });
            xdg.send_pending_configure();
        }
    }

    pub fn render_elements_for_region<R: Renderer + ImportAll, S: Into<Scale<f64>>>(
        &self,
        renderer: &mut R,
        region: &Rectangle<i32, Logical>,
        scale: S,
        alpha: f32,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        R::TextureId: Clone + Texture + 'static,
    {
        let scale = scale.into();

        self.elements
            .iter()
            .rev()
            .filter(|e| {
                let geometry = e.bbox();
                region.overlaps(geometry)
            })
            .flat_map(|e| {
                let location = e.render_location() - region.loc;
                e.element.render_elements(
                    renderer,
                    location.to_physical_precise_round(scale),
                    scale,
                    alpha,
                )
            })
            .collect::<Vec<_>>()
    }

    pub fn render_elements<R: Renderer + ImportAll>(
        &self,
        renderer: &mut R,
        region: &Rectangle<i32, Logical>,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        R::TextureId: Clone + Texture + 'static,
    {
        let mut elements = Vec::new();

        elements.extend(self.render_elements_for_region(renderer, region, scale, 1.0));

        elements
    }
}

impl Default for MyWindowWrapper {
    fn default() -> Self {
        Self::new()
    }
}

pub enum LookForWindowBy<'a> {
    Window(&'a Window),
    Surface(&'a WlSurface),
}
