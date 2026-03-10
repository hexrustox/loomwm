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
    use test_case::test_case;

    use super::*;

    const ARTIFACT_DIR: &str = "/tmp/guide";

    #[test_case(vec![RankingItem {
        app_ids: (1..=10).map(|n| n.to_string()).collect(),
        new: false
    }]; "simple")]
    #[test_case({
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

            items.push(RankingItem { app_ids: ns, new: false });
        }
        items
    }; "random")]
    #[test_case(vec![
        RankingItem {
            app_ids: (1..=10).rev().map(|n| n.to_string()).collect(),
            new: false
        },
        RankingItem {
            app_ids: (1..=10).map(|n| n.to_string()).collect(),
            new: true
        }
    ]; "new_data")]
    fn test_model_identical_dataset(items: Vec<RankingItem>) {
        let device = get_device();
        let dataset = RankingDataset::new(items);

        match device.clone() {
            BackendDevice::Gpu(d) => {
                train::<Autodiff<Wgpu>>(ARTIFACT_DIR.into(), dataset, d);
            }
            BackendDevice::Cpu(d) => {
                train::<Autodiff<NdArray>>(ARTIFACT_DIR.into(), dataset, d);
            }
        }

        let item = RankingItem {
            app_ids: (1..=10).map(|n| n.to_string()).collect(),
            new: false,
        };
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
