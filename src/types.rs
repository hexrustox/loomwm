use smithay::backend::renderer::{ImportAll, Renderer, Texture};

pub trait MyRenderer: Renderer<TextureId = Self::MyTextureId> + ImportAll {
    type MyTextureId: Texture + Clone + 'static;
}

impl<R> MyRenderer for R
where
    R: Renderer + ImportAll,
    R::TextureId: Texture + Clone + 'static,
{
    type MyTextureId = R::TextureId;
}
