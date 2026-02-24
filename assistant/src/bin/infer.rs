use assistant::infer;
use burn::backend::{
    Wgpu,
    wgpu::{
        WgpuDevice,
        graphics::{AutoGraphicsApi, OpenGl},
        init_setup,
    },
};

fn main() {
    type MyBackend = Wgpu<f32, i32>;

    let device = WgpuDevice::default();
    if std::panic::catch_unwind(|| {
        init_setup::<AutoGraphicsApi>(&device, Default::default());
    })
    .is_err()
    {
        println!("Auto initialization failed, falling back to OpenGL");
        init_setup::<OpenGl>(&device, Default::default());
    }

    infer::<MyBackend>("/tmp/guide", device);
}
