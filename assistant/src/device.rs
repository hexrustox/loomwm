use burn::backend::{ndarray::NdArrayDevice, wgpu::WgpuDevice};

#[derive(Debug, Clone)]
pub enum BackendDevice {
    Gpu(WgpuDevice),
    Cpu(NdArrayDevice),
}

pub fn get_device() -> BackendDevice {
    // TODO list gpu
    let has_gpu = !wgpu::Instance::default()
        .enumerate_adapters(wgpu::Backends::all())
        .is_empty();
    if has_gpu {
        BackendDevice::Gpu(WgpuDevice::DefaultDevice)
    } else {
        BackendDevice::Cpu(NdArrayDevice::Cpu)
    }
}
