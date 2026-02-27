use burn::backend::{ndarray::NdArrayDevice, wgpu::WgpuDevice};

pub enum BackendDevice {
    Gpu(WgpuDevice),
    Cpu(NdArrayDevice),
}

pub fn get_device() -> BackendDevice {
    let has_gpu = !wgpu::Instance::default()
        .enumerate_adapters(wgpu::Backends::all())
        .is_empty();
    if has_gpu {
        BackendDevice::Gpu(WgpuDevice::DefaultDevice)
    } else {
        BackendDevice::Cpu(NdArrayDevice::Cpu)
    }
}
