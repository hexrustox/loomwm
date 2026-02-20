use assistant::{RankingDataset, RankingItem, train};
use burn::backend::{
    Autodiff, Wgpu,
    wgpu::{
        WgpuDevice,
        graphics::{AutoGraphicsApi, OpenGl},
        init_setup,
    },
};

fn main() {
    let training_data = vec![
        RankingItem {
            app_ids: vec!["Firefox".to_string(), "VLC".to_string(), "GIMP".to_string()],
        },
        RankingItem {
            app_ids: vec![
                "Chrome".to_string(),
                "LibreOffice".to_string(),
                "Nautilus".to_string(),
                "Blender".to_string(),
            ],
        },
        RankingItem {
            app_ids: vec!["Firefox".to_string(), "Chrome".to_string()],
        },
        RankingItem {
            app_ids: vec![
                "VLC".to_string(),
                "GIMP".to_string(),
                "Thunderbird".to_string(),
                "Audacity".to_string(),
                "Inkscape".to_string(),
            ],
        },
        RankingItem {
            app_ids: vec![
                "Firefox".to_string(),
                "LibreOffice".to_string(),
                "Audacity".to_string(),
            ],
        },
        RankingItem {
            app_ids: vec![
                "Chrome".to_string(),
                "VLC".to_string(),
                "Nautilus".to_string(),
                "Blender".to_string(),
            ],
        },
        RankingItem {
            app_ids: vec![
                "Firefox".to_string(),
                "GIMP".to_string(),
                "Inkscape".to_string(),
            ],
        },
        RankingItem {
            app_ids: vec![
                "LibreOffice".to_string(),
                "VLC".to_string(),
                "Thunderbird".to_string(),
                "Audacity".to_string(),
            ],
        },
        RankingItem {
            app_ids: vec![
                "Firefox".to_string(),
                "Chrome".to_string(),
                "Nautilus".to_string(),
                "Blender".to_string(),
            ],
        },
        RankingItem {
            app_ids: vec![
                "VLC".to_string(),
                "Audacity".to_string(),
                "Inkscape".to_string(),
            ],
        },
    ];
    let testing_data = vec![RankingItem {
        app_ids: vec![
            "GIMP".to_string(),
            "Firefox".to_string(),
            "Audacity".to_string(),
            "Chrome".to_string(),
            "VLC".to_string(),
        ],
    }];

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

    train::<MyAutodiffBackend>(
        "/tmp/guide",
        RankingDataset::new(training_data),
        RankingDataset::new(testing_data),
        device,
    );
}
