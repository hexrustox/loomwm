use std::{
    fs::{create_dir_all, read_to_string},
    thread::spawn,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use assistant::{BackendDevice, RankingDataset, RankingItem, infer, train};
use burn::backend::{Autodiff, NdArray, Wgpu};
use serde::{Deserialize, Serialize};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::{
    monitor::TileTreeWindow, state::WindowManagerState, utils::get_app_id_and_title,
    window::MappedWindow,
};

#[derive(Serialize, Deserialize)]
pub struct LayoutRecord(Vec<RankingItem>);

impl LayoutRecord {
    pub fn read() -> Self {
        if let Ok(str) = read_to_string("/data/model/record.json") {
            serde_json::from_str(&str)
                .map_err(|e| anyhow!("{e}"))
                .unwrap()
        } else {
            Self(Vec::new())
        }
    }

    fn write(&self) {
        let _ = create_dir_all("/data/model");
        let _ = std::fs::write(
            "/data/model/record.json",
            serde_json::to_string(self).unwrap(),
        );
    }
}

impl WindowManagerState {
    pub fn timeout_to_save(&mut self) {
        let is_none = self.save_at.is_none();
        let time = Instant::now() + Duration::from_secs(10);
        self.save_at = Some(time);
        if is_none {
            // TODO
            let _ = self
                .event_loop
                .insert_source(Timer::from_deadline(time), |_, _, data| {
                    let data = &mut data.compositor;

                    let Some(time) = data.save_at else {
                        unreachable!()
                    };
                    if time <= Instant::now() {
                        let mut workspace_windows = Vec::new();
                        for workspace in &data.monitors.get_monitor().workspaces {
                            let mappeds =
                                workspace.tiling_windows_iter().cloned().collect::<Vec<_>>();
                            workspace_windows.push(mappeds);
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

    pub fn append_layout_record(&mut self, items: Vec<MappedWindow>) {
        self.layout_record.0.push(RankingItem {
            app_ids: items
                .into_iter()
                .map(|mapped| {
                    let (app_id, _) = get_app_id_and_title(&mapped.wl_surface());
                    app_id
                })
                .collect::<Vec<_>>(),
        });
        self.event_loop.insert_idle(|data| {
            data.compositor.layout_record.write();
            let items = data.compositor.layout_record.0.clone();
            let device = data.compositor.backend_device.clone();
            spawn(move || match device.lock().unwrap().clone().unwrap() {
                assistant::BackendDevice::Gpu(d) => {
                    train::<Autodiff<Wgpu>>("/data/model", RankingDataset::new(items), d);
                }
                assistant::BackendDevice::Cpu(d) => {
                    train::<Autodiff<NdArray>>("/data/model", RankingDataset::new(items), d);
                }
            });
        });
    }

    pub fn run_assistant(&mut self) {
        let monitor = self.monitors.get_monitor();
        let workspace = monitor.get_workspace(monitor.get_active_workspace_name());
        let iter = workspace.tiling_windows_iter().cloned();
        let mut windows = iter.clone().collect::<Vec<_>>();
        let mut app_ids = iter
            .map(|mapped| {
                let (a, _) = get_app_id_and_title(&mapped.wl_surface());
                a
            })
            .collect::<Vec<_>>();
        let item = RankingItem {
            app_ids: app_ids.clone(),
        };
        let device = self.backend_device.clone();

        spawn(move || {
            let target = match device.lock().unwrap().clone().unwrap() {
                BackendDevice::Gpu(d) => infer::<Wgpu>("/data/model", item, d),
                BackendDevice::Cpu(d) => infer::<NdArray>("/data/model", item, d),
            };
            let ops = get_swap_operations(&mut app_ids, &target);

            for (lhs, rhs) in ops {
                let mut mapped = windows[rhs].clone();
                windows[lhs].swap_location_size(&mut mapped);
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
