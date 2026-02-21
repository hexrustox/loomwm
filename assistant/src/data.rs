use burn::{
    Tensor,
    data::{dataloader::batcher::Batcher, dataset::Dataset},
    prelude::Backend,
    tensor::{Bool, Float, Int, TensorData},
};

use crate::train::Vocab;

#[derive(Clone, Debug)]
pub struct RankingItem {
    pub app_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct RankingBatch<B: Backend> {
    pub inputs: Tensor<B, 3, Int>,
    pub targets: Tensor<B, 2, Float>,
    pub word_mask: Tensor<B, 3, Bool>,
    pub list_mask: Tensor<B, 2, Bool>,
}

#[derive(Clone)]
pub struct RankingBatcher {
    vocab: Vocab,
}

impl RankingBatcher {
    pub fn new(vocab: Vocab) -> Self {
        Self { vocab }
    }
}

impl<B: Backend> Batcher<B, RankingItem, RankingBatch<B>> for RankingBatcher {
    fn batch(&self, items: Vec<RankingItem>, device: &B::Device) -> RankingBatch<B> {
        let batch_size = items.len();
        let max_list_len = items
            .iter()
            .map(|item| item.app_ids.len())
            .max()
            .unwrap_or(0);

        let max_str_len = 10;

        // 1. Pre-allocate flat vectors with exact capacity
        let mut inputs_data = vec![0i32; batch_size * max_list_len * max_str_len];
        let mut labels_data = vec![0.0f32; batch_size * max_list_len];
        let mut list_mask_data = vec![true; batch_size * max_list_len]; // Default to masked
        let mut word_mask_data = vec![true; batch_size * max_list_len * max_str_len]; // Default to masked

        // 2. Single pass iteration
        for (b, item) in items.into_iter().enumerate() {
            let list_offset = b * max_list_len;

            for (i, app_id) in item.app_ids.into_iter().take(max_list_len).enumerate() {
                let current_list_idx = list_offset + i;
                let word_offset = current_list_idx * max_str_len;

                // 3. Tokenize directly into the slice if your vocab allows,
                // otherwise encode and copy.
                let tokens = self.vocab.encode(&app_id, max_str_len);

                for (j, &tok) in tokens.iter().enumerate() {
                    let idx = word_offset + j;
                    inputs_data[idx] = tok as i32;
                    word_mask_data[idx] = tok == 0; // Assuming 0 is padding
                }

                labels_data[current_list_idx] = i as f32;
                list_mask_data[current_list_idx] = false; // Unmask valid items
            }
        }

        // 4. Convert to Tensors using i32 (standard for Burn Int tensors)
        let inputs = Tensor::<B, 3, Int>::from_data(
            TensorData::new(inputs_data, [batch_size, max_list_len, max_str_len]),
            device,
        );
        let labels = Tensor::<B, 2, Float>::from_data(
            TensorData::new(labels_data, [batch_size, max_list_len]),
            device,
        );
        let word_mask = Tensor::<B, 3, Bool>::from_data(
            TensorData::new(word_mask_data, [batch_size, max_list_len, max_str_len]),
            device,
        );
        let list_mask = Tensor::<B, 2, Bool>::from_data(
            TensorData::new(list_mask_data, [batch_size, max_list_len]),
            device,
        );

        RankingBatch {
            inputs,
            targets: labels,
            word_mask,
            list_mask,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RankingDataset {
    items: Vec<RankingItem>,
}

impl RankingDataset {
    pub fn new(items: Vec<RankingItem>) -> Self {
        Self { items }
    }

    pub fn train() -> Self {
        Self {
            items: vec![
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
            ],
        }
    }

    pub fn test() -> Self {
        Self {
            // expect: ["Firefox", "Chrome", "VLC", "GIMP", "Audacity"]
            items: vec![
                RankingItem {
                    app_ids: vec![
                        "GIMP".to_string(),
                        "Firefox".to_string(),
                        "Audacity".to_string(),
                        "Chrome".to_string(),
                        "VLC".to_string(),
                    ],
                },
                RankingItem {
                    app_ids: vec!["Chrome".to_string(), "Firefox".to_string()],
                },
            ],
        }
    }
}

impl Dataset<RankingItem> for RankingDataset {
    fn get(&self, index: usize) -> Option<RankingItem> {
        self.items.get(index).cloned()
    }

    fn len(&self) -> usize {
        self.items.len()
    }
}
