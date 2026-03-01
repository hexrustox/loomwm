use std::{
    fs::{create_dir_all, read_to_string},
    thread::spawn,
};

use anyhow::anyhow;
use assistant::{RankingDataset, RankingItem, get_device, train};
use burn::backend::{Autodiff, NdArray, Wgpu};
use serde::{Deserialize, Serialize};

use crate::{state::WindowManagerState, utils::get_app_id_and_title, window::MappedWindow};

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
            spawn(move || {
                println!("start train");

                let device = get_device();
                println!("{device:?}");
                match device {
                    assistant::BackendDevice::Gpu(d) => {
                        train::<Autodiff<Wgpu>>("/data/model", RankingDataset::new(items), d);
                    }
                    assistant::BackendDevice::Cpu(d) => {
                        train::<Autodiff<NdArray>>("/data/model", RankingDataset::new(items), d);
                    }
                }

                println!("stop train");
            });
        });
    }
}
