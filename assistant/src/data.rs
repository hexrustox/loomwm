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

        let mut inputs_data = Vec::with_capacity(batch_size * max_list_len * max_str_len);
        let mut labels_data = Vec::with_capacity(batch_size * max_list_len);
        let mut list_mask_data = Vec::with_capacity(batch_size * max_list_len);
        let mut word_mask_data = Vec::with_capacity(batch_size * max_list_len * max_str_len);

        for RankingItem { app_ids } in items {
            for i in 0..max_list_len {
                if i < app_ids.len() {
                    let app_id = &app_ids[i];
                    let tokens = self.vocab.encode(app_id, max_str_len);

                    for &tok in &tokens {
                        inputs_data.push(tok as i64);
                        word_mask_data.push(tok == 0);
                    }

                    labels_data.push(i as f32);
                    list_mask_data.push(false);
                } else {
                    inputs_data.extend(vec![0; max_str_len]);
                    word_mask_data.extend(vec![true; max_str_len]);

                    labels_data.push(0.0);
                    list_mask_data.push(true);
                }
            }
        }

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
}

impl Dataset<RankingItem> for RankingDataset {
    fn get(&self, index: usize) -> Option<RankingItem> {
        self.items.get(index).cloned()
    }

    fn len(&self) -> usize {
        self.items.len()
    }
}
