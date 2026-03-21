#![recursion_limit = "256"]

mod data;
mod device;
mod infer;
mod model;
mod train;

pub use data::{RankingDataset, RankingItem};
pub use device::{BackendDevice, get_device};
pub use infer::infer;
pub use train::train;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use burn::backend::{Autodiff, NdArray, Wgpu};
    use rand::seq::SliceRandom;
    use test_case::test_case;

    use super::*;

    const ARTIFACT_DIR: &str = "/tmp/guide";

    fn run_training(device: BackendDevice, artifact_dir: PathBuf, dataset: RankingDataset) {
        match device {
            BackendDevice::Gpu(d) => train::<Autodiff<Wgpu>>(artifact_dir, dataset, d),
            BackendDevice::Cpu(d) => train::<Autodiff<NdArray>>(artifact_dir, dataset, d),
        }
    }

    fn run_inference(
        artifact_dir: PathBuf,
        item: RankingItem,
        device: BackendDevice,
    ) -> Vec<String> {
        match device {
            BackendDevice::Gpu(d) => infer::<Wgpu>(artifact_dir, item, d),
            BackendDevice::Cpu(d) => infer::<NdArray>(artifact_dir, item, d),
        }
        .unwrap()
    }

    #[test_case(3, vec![RankingItem {
        app_ids: (1..=3).map(|n| n.to_string()).collect(),
    }]; "short_list")]
    #[test_case(8, vec![RankingItem {
        app_ids: (1..=8).map(|n| n.to_string()).collect(),
    }]; "medium_list")]
    #[test_case(15, vec![RankingItem {
        app_ids: (1..=15).map(|n| n.to_string()).collect(),
    }]; "long_list")]
    #[test_case(10, {
        let mut items = Vec::new();
        for _ in 0..10 {
            let mut ns = (1..=10).map(|n| n.to_string()).collect::<Vec<_>>();
            for _ in 0..rand::random_range(2..4) {
                ns.remove(rand::random_range(0..ns.len()));
            }
            assert!(ns.len() < 10);
            assert!(
                ns.clone()
                    .into_iter()
                    .map(|s| s.parse::<usize>().unwrap())
                    .is_sorted()
            );

            items.push(RankingItem { app_ids: ns });
        }
        items
    }; "random")]
    fn test_model_sequence_dataset(max: usize, items: Vec<RankingItem>) {
        let device = get_device();

        println!("{items:#?}");

        let dataset = RankingDataset::new(items);
        run_training(device.clone(), ARTIFACT_DIR.into(), dataset);

        let mut app_ids: Vec<_> = (1..=max).collect();
        while app_ids.is_sorted() {
            app_ids.shuffle(&mut rand::rng());
        }
        println!("{app_ids:?}");
        let app_ids = app_ids.iter().map(|n| n.to_string()).collect();

        let item = RankingItem { app_ids };
        let result = run_inference(ARTIFACT_DIR.into(), item, device)
            .into_iter()
            .map(|s| s.parse::<usize>().unwrap())
            .collect::<Vec<_>>();

        println!("{result:?}");
        assert!(result.is_sorted());
    }

    #[test]
    fn test_model_similar_app_id() {
        let device = get_device();

        let dataset = RankingDataset::new(vec![
            RankingItem {
                app_ids: vec![
                    "editor A".to_string(),
                    "terminal".to_string(),
                    "browser".to_string(),
                ],
            },
            RankingItem {
                app_ids: vec![
                    "editor B".to_string(),
                    "terminal".to_string(),
                    "browser".to_string(),
                ],
            },
        ]);

        run_training(device.clone(), ARTIFACT_DIR.into(), dataset);

        let app_ids = vec![
            "editor C".to_string(),
            "terminal".to_string(),
            "browser".to_string(),
        ];
        let mut app_ids_clone = app_ids.clone();
        while app_ids_clone == app_ids {
            app_ids_clone.shuffle(&mut rand::rng());
        }
        println!("{app_ids_clone:?}");

        let item = RankingItem {
            app_ids: app_ids_clone,
        };

        let result = run_inference(ARTIFACT_DIR.into(), item, device);

        println!("{result:?}");
        assert_eq!(result, app_ids)
    }
}
