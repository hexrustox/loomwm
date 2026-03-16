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
    use burn::backend::{Autodiff, NdArray, Wgpu};
    use rand::seq::SliceRandom;
    use test_case::test_case;

    use super::*;

    const ARTIFACT_DIR: &str = "/tmp/guide";

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
    fn test_model_identical_dataset(max: usize, items: Vec<RankingItem>) {
        let device = get_device();

        println!("{items:#?}");

        let dataset = RankingDataset::new(items);

        match device.clone() {
            BackendDevice::Gpu(d) => {
                train::<Autodiff<Wgpu>>(ARTIFACT_DIR.into(), dataset, d);
            }
            BackendDevice::Cpu(d) => {
                train::<Autodiff<NdArray>>(ARTIFACT_DIR.into(), dataset, d);
            }
        }

        let mut app_ids: Vec<_> = (1..=max).collect();
        while app_ids.is_sorted() {
            app_ids.shuffle(&mut rand::rng());
        }
        println!("{app_ids:?}");
        let app_ids = app_ids.iter().map(|n| n.to_string()).collect();

        let item = RankingItem { app_ids };
        let result = match device {
            BackendDevice::Gpu(d) => infer::<Wgpu>(ARTIFACT_DIR.into(), item, d),
            BackendDevice::Cpu(d) => infer::<NdArray>(ARTIFACT_DIR.into(), item, d),
        }
        .unwrap()
        .into_iter()
        .map(|s| s.parse::<usize>().unwrap())
        .collect::<Vec<_>>();

        println!("{result:?}");
        assert!(result.is_sorted());
    }
}
