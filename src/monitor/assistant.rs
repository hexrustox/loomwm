use std::{
    fs::{create_dir_all, read_to_string},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread::spawn,
    time::{Duration, Instant},
};

use assistant::{
    BackendDevice as InnerBackendDevice, RankingDataset, RankingItem, get_device, infer, train,
};
use burn::backend::{Autodiff, NdArray, Wgpu};
use burn::data::dataloader::Dataset;
use serde::{Deserialize, Serialize};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use tracing::{error, info};

use crate::{path::model_dir, state::WindowManagerState, utils::get_app_id_and_title};

#[derive(Clone)]
pub struct BackendDevice {
    device: Arc<Mutex<Option<InnerBackendDevice>>>,
    ready: Arc<Condvar>,
}

impl BackendDevice {
    pub fn new() -> Self {
        Self {
            device: Arc::new(Mutex::new(None)),
            ready: Arc::new(Condvar::new()),
        }
    }

    pub fn init(&mut self) {
        if self.device.lock().unwrap().is_some() {
            return;
        }

        let device = self.device.clone();
        let ready = self.ready.clone();
        spawn(move || {
            *device.lock().unwrap() = Some(get_device());
            ready.notify_all();
            info!("Init assistant device");
        });
    }

    fn get_device(&self) -> InnerBackendDevice {
        let mut guard = self.device.lock().unwrap();
        loop {
            if let Some(device) = guard.as_ref().cloned() {
                return device;
            }
            guard = self.ready.wait(guard).unwrap();
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct LayoutHistory {
    dataset: RankingDataset,
    buffer: Vec<Arc<RankingItem>>,
}

impl LayoutHistory {
    fn path() -> PathBuf {
        model_dir().join("history.json")
    }

    pub fn read() -> Self {
        if let Some(this) = read_to_string(Self::path())
            .ok()
            .and_then(|content| serde_json::from_str::<Self>(&content).ok())
        {
            this
        } else {
            Self {
                dataset: RankingDataset::new(Vec::new()),
                buffer: Vec::new(),
            }
        }
    }

    fn save(&self) {
        if let Some(parent) = Self::path().parent() {
            let _ = create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(Self::path(), serde_json::to_string(self).unwrap()) {
            error!("Failed to write history: {e}");
        }
    }

    fn push_buffer(&mut self, item: RankingItem) {
        let item = Arc::new(item);
        if self
            .dataset
            .iter()
            .map(|(i, _)| i)
            .chain(self.buffer.clone())
            .any(|i| i == item)
        {
            return;
        }
        self.buffer.push(item);
    }
}

impl WindowManagerState {
    pub fn save_layout_history(&mut self) {
        if !self.assistant_config.enable {
            return;
        }

        let is_none = self.save_at.is_none();
        let time = Instant::now() + Duration::from_secs(self.assistant_config.save_layout_after);
        self.save_at = Some(time);
        if is_none {
            let _ = self
                .event_loop
                .insert_source(Timer::from_deadline(time), move |_, _, data| {
                    let data = &mut data.compositor;

                    if time <= Instant::now() {
                        let mut workspace_windows = Vec::new();
                        for workspace in &data.monitors.get_monitor().workspaces {
                            let mappeds =
                                workspace.tiling_windows_iter().cloned().collect::<Vec<_>>();
                            if mappeds.len() > 1 {
                                workspace_windows.push(mappeds);
                            }
                        }
                        for items in workspace_windows {
                            data.layout_history.push_buffer(RankingItem {
                                app_ids: items
                                    .into_iter()
                                    .map(|mapped| {
                                        let (app_id, _) =
                                            get_app_id_and_title(&mapped.wl_surface());
                                        app_id
                                    })
                                    .collect::<Vec<_>>(),
                            });

                            if data.layout_history.buffer.len()
                                >= data.assistant_config.buffer_length
                            {
                                // TODO drain buffer put into dataset, train, set all to old, save.
                                for item in data.layout_history.buffer.drain(..) {
                                    data.layout_history.dataset.enqueue(item);
                                }

                                data.event_loop.insert_idle(|data| {
                                    let data = &mut data.compositor;

                                    let dataset = data.layout_history.dataset.clone();
                                    let device = data.backend_device.clone();
                                    spawn(move || match device.get_device() {
                                        InnerBackendDevice::Gpu(d) => {
                                            train::<Autodiff<Wgpu>>(model_dir(), dataset, d);
                                        }
                                        InnerBackendDevice::Cpu(d) => {
                                            train::<Autodiff<NdArray>>(model_dir(), dataset, d);
                                        }
                                    });
                                });
                            }

                            data.layout_history.save();
                            info!("Layout history saved");
                        }

                        data.save_at = None;
                        TimeoutAction::Drop
                    } else {
                        data.save_at = Some(time);
                        TimeoutAction::ToInstant(time)
                    }
                });
        }
    }

    pub fn run_assistant(&mut self) {
        if !self.assistant_config.enable {
            return;
        }

        let monitor = self.monitors.get_monitor();
        let workspace = monitor.get_workspace(monitor.get_active_workspace_name());
        let iter = workspace.tiling_windows_iter().cloned();
        let mappeds = iter.clone().collect::<Vec<_>>();

        if mappeds.len() <= 1 {
            return;
        }

        // TODO arc
        let mut app_ids = iter
            .map(|mapped| {
                let (app_id, _) = get_app_id_and_title(&mapped.wl_surface());
                app_id
            })
            .collect::<Vec<_>>();
        let item = RankingItem {
            app_ids: app_ids.clone(),
        };
        let device = self.backend_device.clone();

        let ops = spawn(move || {
            let target = match device.get_device() {
                InnerBackendDevice::Gpu(d) => infer::<Wgpu>(model_dir(), item, d),
                InnerBackendDevice::Cpu(d) => infer::<NdArray>(model_dir(), item, d),
            };
            let Ok(target) = target else {
                error!("Assistant failed: {}", target.unwrap_err());
                return Vec::new();
            };

            get_swap_operations(&mut app_ids, &target)
        })
        .join()
        .unwrap();

        for (lhs, rhs) in ops {
            self.swap_tiling_window(&mappeds[lhs].wl_surface(), &mappeds[rhs].wl_surface());
        }
    }
}

pub fn get_swap_operations<T: PartialEq>(list: &mut [T], target: &[T]) -> Vec<(usize, usize)> {
    let mut ops = Vec::new();

    for i in 0..list.len() {
        let j = target
            .iter()
            .position(|e| *e == list[i])
            .expect("lists have different elements");
        if i == j {
            continue;
        }
        ops.push((i, j));
        list.swap(j, i);
    }

    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(vec![])]
    #[test_case(vec![1, 3, 2])]
    #[test_case(vec![1, 2, 1])]
    fn test_get_swap_operations(mut list: Vec<i32>) {
        let mut target = list.clone();
        target.sort();
        get_swap_operations(&mut list, &target);
        assert!(list.is_sorted());
    }
}
