use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use burn::{
    Tensor,
    data::{dataloader::batcher::Batcher, dataset::Dataset},
    prelude::Backend,
    tensor::{Bool, Float, Int, TensorData},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Vocab {
    string_to_token: HashMap<String, usize>,
}

impl Vocab {
    pub fn new(data: impl Iterator<Item = String>) -> Self {
        let mut string_to_token = HashMap::new();
        string_to_token.insert("<PAD>".to_string(), 0);
        string_to_token.insert("<UNK>".to_string(), 1);

        data.for_each(|str| {
            for word in str.to_lowercase().split_whitespace() {
                let len = string_to_token.len();
                string_to_token.entry(word.to_string()).or_insert(len);
            }
        });

        Self { string_to_token }
    }

    pub fn encode(&self, text: &str, max_len: usize) -> Vec<usize> {
        let mut tokens: Vec<usize> = text
            .to_lowercase()
            .split_whitespace()
            .take(max_len)
            .map(|w| *self.string_to_token.get(w).unwrap_or(&1))
            .collect();

        tokens.resize(max_len, 0);
        tokens
    }

    pub fn vocab_size(&self) -> usize {
        self.string_to_token.len()
    }
}

// TODO train on app title
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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

pub struct RankingBatcher {
    vocab: Vocab,
}

impl RankingBatcher {
    pub fn new(vocab: Vocab) -> Self {
        Self { vocab }
    }
}

impl<B: Backend> Batcher<B, Arc<RankingItem>, RankingBatch<B>> for RankingBatcher {
    fn batch(&self, items: Vec<Arc<RankingItem>>, device: &B::Device) -> RankingBatch<B> {
        let batch_size = items.len();
        let max_list_len = items
            .iter()
            .map(|item| item.app_ids.len())
            .max()
            .unwrap_or(0);

        let max_str_len = 10;

        let mut inputs_data = vec![0i32; batch_size * max_list_len * max_str_len];
        let mut labels_data = vec![0.0f32; batch_size * max_list_len];
        let mut list_mask_data = vec![true; batch_size * max_list_len];
        let mut word_mask_data = vec![true; batch_size * max_list_len * max_str_len];

        for (b, item) in items.into_iter().enumerate() {
            let list_offset = b * max_list_len;

            for (i, app_id) in item
                .app_ids
                .clone()
                .into_iter()
                .take(max_list_len)
                .enumerate()
            {
                let current_list_idx = list_offset + i;
                let word_offset = current_list_idx * max_str_len;

                let tokens = self.vocab.encode(&app_id, max_str_len);

                for (j, &tok) in tokens.iter().enumerate() {
                    let idx = word_offset + j;
                    inputs_data[idx] = tok as i32;
                    word_mask_data[idx] = tok == 0;
                }

                labels_data[current_list_idx] = i as f32;
                list_mask_data[current_list_idx] = false;
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RankingDataset {
    items: VecDeque<Arc<RankingItem>>,
}

impl RankingDataset {
    pub fn new(items: Vec<RankingItem>) -> Self {
        Self {
            items: items.into_iter().map(Arc::new).collect(),
        }
    }

    pub fn enqueue(&mut self, item: RankingItem, limit: usize) {
        if let Some(index) = self.items.iter().position(|i| **i == item) {
            self.items.remove(index);
        }
        self.items.push_back(Arc::new(item));
        if self.items.len() > limit {
            self.items.pop_front();
        }
    }
}

impl Dataset<Arc<RankingItem>> for RankingDataset {
    fn get(&self, index: usize) -> Option<Arc<RankingItem>> {
        self.items.get(index).cloned()
    }

    fn len(&self) -> usize {
        self.items.len()
    }
}
