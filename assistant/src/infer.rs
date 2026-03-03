use std::{fs::read_to_string, iter::zip, path::PathBuf, sync::Arc};

use burn::{
    config::Config,
    data::dataloader::batcher::Batcher,
    module::Module,
    prelude::Backend,
    record::{CompactRecorder, Recorder},
    tensor::activation::softmax,
};

use crate::{
    RankingItem,
    data::{RankingBatch, RankingBatcher, Vocab},
    train::TrainingConfig,
};

pub fn infer<B: Backend>(model_dir: PathBuf, item: RankingItem, device: B::Device) -> Vec<String> {
    let vocab =
        serde_json::from_str::<Vocab>(&read_to_string(model_dir.join("vocab.json")).unwrap())
            .unwrap();
    let config = TrainingConfig::load(model_dir.join("config.json")).unwrap();
    let record = CompactRecorder::new()
        .load(model_dir.join("model"), &device)
        .unwrap();

    let model = config.model.init::<B>(&device).load_record(record);

    let batcher = RankingBatcher::new(vocab);
    let app_ids = item.app_ids.clone();
    let batch: RankingBatch<B> = batcher.batch(vec![Arc::new(item)], &device);

    let output = model.forward(batch.inputs, batch.word_mask, batch.list_mask);
    let output: Vec<f32> = softmax(output, 1)
        .reshape([-1])
        .into_data()
        .as_slice()
        .unwrap()
        .to_vec();

    let mut result = zip(app_ids, output).collect::<Vec<_>>();
    result.sort_by(|(_, a), (_, b)| {
        use std::cmp::Ordering::*;
        if a < b {
            Less
        } else if a > b {
            Greater
        } else {
            Equal
        }
    });
    result.into_iter().map(|(s, _)| s).collect()
}
