use std::{
    fs::{create_dir_all, read_to_string},
    sync::{Arc, Condvar, Mutex},
    thread::spawn,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use assistant::{
    BackendDevice as InnerBackendDevice, RankingDataset, RankingItem, get_device, infer, train,
};
use burn::backend::{Autodiff, NdArray, Wgpu};
use burn::data::dataloader::Dataset;
use serde::{Deserialize, Serialize};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::{
    monitor::TileTreeWindow, state::WindowManagerState, utils::get_app_id_and_title,
    window::MappedWindow,
};

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
        *self.device.lock().unwrap() = Some(get_device());
        self.ready.notify_all();
    }

    fn get_device(&self) -> InnerBackendDevice {
        let mut guard = self.device.lock().unwrap();
        while guard.is_none() {
            guard = self.ready.wait(guard).unwrap();
        }

        guard.as_ref().cloned().unwrap()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayoutRecord(RankingDataset);

impl LayoutRecord {
    pub fn read() -> Self {
        if let Ok(str) = read_to_string("/data/model/record.json") {
            serde_json::from_str(&str)
                .map_err(|e| anyhow!("{e}"))
                .unwrap()
        } else {
            Self(RankingDataset::new(Vec::new()))
        }
    }

    fn write(&self) {
        let _ = create_dir_all("/data/model");
        let _ = std::fs::write(
            "/data/model/record.json",
            serde_json::to_string(self).unwrap(),
        );
    }

    fn push(&mut self, item: RankingItem, limit: usize) {
        self.0.enqueue(item);
        if self.0.len() > limit {
            self.0.dequeue()
        }
    }

    fn dataset(&self) -> RankingDataset {
        self.0.clone()
    }
}

impl WindowManagerState {
    pub fn timeout_to_save(&mut self) {
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
                            data.append_layout_record(items);
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

    fn append_layout_record(&mut self, items: Vec<MappedWindow>) {
        self.layout_record.push(
            RankingItem {
                app_ids: items
                    .into_iter()
                    .map(|mapped| {
                        let (app_id, _) = get_app_id_and_title(&mapped.wl_surface());
                        app_id
                    })
                    .collect::<Vec<_>>(),
            },
            self.assistant_config.history_length,
        );

        self.event_loop.insert_idle(|data| {
            data.compositor.layout_record.write();
            let dataset = data.compositor.layout_record.dataset();
            let device = data.compositor.backend_device.clone();
            spawn(move || match device.get_device() {
                InnerBackendDevice::Gpu(d) => {
                    train::<Autodiff<Wgpu>>("/data/model", dataset, d);
                }
                InnerBackendDevice::Cpu(d) => {
                    train::<Autodiff<NdArray>>("/data/model", dataset, d);
                }
            });
        });
    }

    pub fn run_assistant(&mut self) {
        if !self.assistant_config.enable {
            return;
        }

        let monitor = self.monitors.get_monitor();
        let workspace = monitor.get_workspace(monitor.get_active_workspace_name());
        let iter = workspace.tiling_windows_iter().cloned();
        let mut mappeds = iter.clone().collect::<Vec<_>>();

        if mappeds.len() <= 1 {
            return;
        }

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

        spawn(move || {
            let target = match device.get_device() {
                InnerBackendDevice::Gpu(d) => infer::<Wgpu>("/data/model", item, d),
                InnerBackendDevice::Cpu(d) => infer::<NdArray>("/data/model", item, d),
            };
            let ops = get_swap_operations(&mut app_ids, &target);

            for (lhs, rhs) in ops {
                let mut mapped = mappeds[rhs].clone();
                mappeds[lhs].swap_location_size(&mut mapped);
            }
        });
    }
}

pub fn get_swap_operations<T: PartialEq>(list: &mut [T], target: &[T]) -> Vec<(usize, usize)> {
    let mut ops = Vec::new();

    for i in 0..list.len() {
        let j = target.iter().position(|e| *e == list[i]).unwrap();
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
