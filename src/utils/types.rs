use smithay::{
    backend::renderer::{
        ImportAll, Renderer as SmithayRenderer, RendererSuper,
        element::{
            solid::SolidColorRenderElement, surface::WaylandSurfaceRenderElement,
            utils::CropRenderElement,
        },
    },
    render_elements,
};

render_elements! {
    pub RenderElements<R> where R: SmithayRenderer + ImportAll;
    Window=CropRenderElement<WaylandSurfaceRenderElement<R>>,
    Border=SolidColorRenderElement
}

pub trait Renderer: SmithayRenderer + ImportAll
where
    <Self as RendererSuper>::TextureId: Clone + 'static,
{
}

impl<R> Renderer for R
where
    R: SmithayRenderer + ImportAll,
    <R as RendererSuper>::TextureId: Clone + 'static,
{
}
