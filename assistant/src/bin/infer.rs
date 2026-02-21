use assistant::{RankingDataset, infer};
use burn::backend::{
    Autodiff, Wgpu,
    wgpu::{
        WgpuDevice,
        graphics::{AutoGraphicsApi, OpenGl},
        init_setup,
    },
};

fn main() {
    type MyBackend = Wgpu<f32, i32>;
    type MyAutodiffBackend = Autodiff<MyBackend>;

    let device = WgpuDevice::default();
    if std::panic::catch_unwind(|| {
        init_setup::<AutoGraphicsApi>(&device, Default::default());
    })
    .is_err()
    {
        println!("Auto initialization failed, falling back to OpenGL");
        init_setup::<OpenGl>(&device, Default::default());
    }

    infer::<MyAutodiffBackend>(
        "/tmp/guide",
        RankingDataset::train(),
        RankingDataset::test(),
        device,
    );
}
