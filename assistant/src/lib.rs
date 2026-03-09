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
    }])]
    fn test_model(items: Vec<RankingItem>) {
        tracing_subscriber::fmt().init();

        println!("{items:#?}");

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
        };
        let result = match device {
            BackendDevice::Gpu(d) => infer::<Wgpu>(ARTIFACT_DIR.into(), item, d),
            BackendDevice::Cpu(d) => infer::<NdArray>(ARTIFACT_DIR.into(), item, d),
        }
        .unwrap()
        .into_iter()
        .map(|s| s.parse::<usize>().unwrap())
        .collect::<Vec<_>>();

        println!("{result:#?}");
        assert!(result.is_sorted());
    }
}
