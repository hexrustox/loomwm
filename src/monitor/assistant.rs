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
use serde::{Deserialize, Serialize};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use tracing::{error, info};

use crate::{path::model_dir, state::WindowManagerState, utils::get_app_id_and_title};

#[derive(Clone)]
pub struct BackendDevice {
    device: Arc<Mutex<Option<InnerBackendDevice>>>,
    ready: Arc<Condvar>,
}

impl Default for BackendDevice {
    fn default() -> Self {
        Self::new()
    }
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

pub struct LayoutHistory(Arc<Mutex<InnerLayoutHistory>>);

impl Default for LayoutHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutHistory {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(InnerLayoutHistory::read())))
    }

    fn push(&mut self, item: RankingItem, limit: usize) {
        let mut guard = self.0.lock().unwrap();
        guard.dataset.enqueue(item, limit);
    }

    fn get_dataset(&self) -> RankingDataset {
        let guard = self.0.lock().unwrap();
        guard.dataset.clone()
    }

    fn save(&self) {
        let guard = self.0.lock().unwrap();
        guard.save();
    }
}

impl Clone for LayoutHistory {
    fn clone(&self) -> Self {
        LayoutHistory(self.0.clone())
    }
}

#[derive(Serialize, Deserialize)]
struct InnerLayoutHistory {
    dataset: RankingDataset,
}

impl InnerLayoutHistory {
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

                    if let Some(time) = data.save_at {
                        if time <= Instant::now() {
                            data.train_model();

                            data.save_at = None;
                            TimeoutAction::Drop
                        } else {
                            data.save_at = Some(time);
                            TimeoutAction::ToInstant(time)
                        }
                    } else {
                        data.save_at = None;
                        TimeoutAction::Drop
                    }
                });
        }
    }

    fn train_model(&self) {
        let mut workspace_windows = Vec::new();
        for workspace in &self.monitors.get_monitor().workspaces {
            let mappeds = workspace.tiling_windows_iter().cloned().collect::<Vec<_>>();
            if mappeds.len() > 1 {
                workspace_windows.push(mappeds);
            }
        }
        for items in workspace_windows {
            let mut history = self.layout_history.clone();
            history.push(
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
            history.save();
            info!("Layout history saved");

            let device = self.backend_device.clone();

            spawn(move || {
                let dataset = history.get_dataset();

                match device.get_device() {
                    InnerBackendDevice::Gpu(d) => {
                        train::<Autodiff<Wgpu>>(model_dir(), dataset, d);
                    }
                    InnerBackendDevice::Cpu(d) => {
                        train::<Autodiff<NdArray>>(model_dir(), dataset, d);
                    }
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
        let result = match device.get_device() {
            InnerBackendDevice::Gpu(d) => infer::<Wgpu>(model_dir(), item, d),
            InnerBackendDevice::Cpu(d) => infer::<NdArray>(model_dir(), item, d),
        };
        match result {
            Ok(target) => {
                self.save_at = None;

                let ops = get_swap_operations(&mut app_ids, &target);

                for (lhs, rhs) in ops {
                    self.swap_tiling_window(&mappeds[lhs].wl_surface(), &mappeds[rhs].wl_surface());
                }
            }
            Err(e) => {
                error!("Assistant failed: {e}");
            }
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
