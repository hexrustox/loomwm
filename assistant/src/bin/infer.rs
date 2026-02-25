use assistant::{get_device, infer};
use burn::backend::{NdArray, Wgpu};

fn main() {
    match get_device() {
        assistant::BackendDevice::Gpu(d) => {
            infer::<Wgpu>("/tmp/guide", d);
        }
        assistant::BackendDevice::Cpu(d) => {
            infer::<NdArray>("/tmp/guide", d);
        }
    }
}
