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

    use super::*;

    const ARTIFACT_DIR: &str = "/tmp/guide";

    #[test]
    fn test_model() {
        let items = vec![
            RankingItem {
                app_ids: vec!["Firefox".to_string(), "VLC".to_string(), "GIMP".to_string()],
            },
            RankingItem {
                app_ids: vec![
                    "Chrome".to_string(),
                    "LibreOffice".to_string(),
                    "Nautilus".to_string(),
                    "Blender".to_string(),
                ],
            },
            RankingItem {
                app_ids: vec!["Firefox".to_string(), "Chrome".to_string()],
            },
            RankingItem {
                app_ids: vec![
                    "VLC".to_string(),
                    "GIMP".to_string(),
                    "Thunderbird".to_string(),
                    "Audacity".to_string(),
                    "Inkscape".to_string(),
                ],
            },
            RankingItem {
                app_ids: vec![
                    "Firefox".to_string(),
                    "LibreOffice".to_string(),
                    "Audacity".to_string(),
                ],
            },
            RankingItem {
                app_ids: vec![
                    "Chrome".to_string(),
                    "VLC".to_string(),
                    "Nautilus".to_string(),
                    "Blender".to_string(),
                ],
            },
            RankingItem {
                app_ids: vec![
                    "Firefox".to_string(),
                    "GIMP".to_string(),
                    "Inkscape".to_string(),
                ],
            },
            RankingItem {
                app_ids: vec![
                    "LibreOffice".to_string(),
                    "VLC".to_string(),
                    "Thunderbird".to_string(),
                    "Audacity".to_string(),
                ],
            },
            RankingItem {
                app_ids: vec![
                    "Firefox".to_string(),
                    "Chrome".to_string(),
                    "Nautilus".to_string(),
                    "Blender".to_string(),
                ],
            },
            RankingItem {
                app_ids: vec![
                    "VLC".to_string(),
                    "Audacity".to_string(),
                    "Inkscape".to_string(),
                ],
            },
        ];

        let device = get_device();
        let dataset = RankingDataset::new(items);

        match device.clone() {
            BackendDevice::Gpu(d) => {
                train::<Autodiff<Wgpu>>(ARTIFACT_DIR, dataset, d);
            }
            BackendDevice::Cpu(d) => {
                train::<Autodiff<NdArray>>(ARTIFACT_DIR, dataset, d);
            }
        }

        let item = RankingItem {
            app_ids: ["Chrome", "Inkscape", "Firefox"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        };
        let result = match device {
            BackendDevice::Gpu(d) => infer::<Wgpu>(ARTIFACT_DIR, item, d),
            BackendDevice::Cpu(d) => infer::<NdArray>(ARTIFACT_DIR, item, d),
        };
        assert_eq!(result[0], "Firefox");
        assert_eq!(result[1], "Chrome");
        assert_eq!(result[2], "Inkscape");
        assert_eq!(result.len(), 3);
    }
}
