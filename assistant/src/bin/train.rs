use assistant::{RankingDataset, get_device, train};
use burn::backend::{Autodiff, NdArray, Wgpu};

fn main() {
    match get_device() {
        assistant::BackendDevice::Gpu(d) => {
            train::<Autodiff<Wgpu>>("/tmp/guide", RankingDataset::train(), d);
        }
        assistant::BackendDevice::Cpu(d) => {
            train::<Autodiff<NdArray>>("/tmp/guide", RankingDataset::train(), d);
        }
    }
}
