use std::{fs::read_to_string, iter::zip};

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
    data::{RankingBatcher, Vocab},
    train::TrainingConfig,
};

pub fn infer<B: Backend>(artifact_dir: &str, device: B::Device) {
    let vocab = serde_json::from_str::<Vocab>(
        &read_to_string(format!("{artifact_dir}/vocab.json")).unwrap(),
    )
    .unwrap();
    let config = TrainingConfig::load(format!("{artifact_dir}/config.json")).unwrap();
    let record = CompactRecorder::new()
        .load(format!("{artifact_dir}/model").into(), &device)
        .unwrap();

    let model = config.model.init::<B>(&device).load_record(record);

    let batcher = RankingBatcher::new(vocab);
    let batch: crate::data::RankingBatch<B> = batcher.batch(
        vec![RankingItem {
            app_ids: vec![
                "GIMP".to_string(),
                "Firefox".to_string(),
                "Audacity".to_string(),
                "Chrome".to_string(),
                "VLC".to_string(),
            ],
        }],
        &device,
    );
    let output = model.forward(batch.inputs, batch.word_mask, batch.list_mask);
    let output: Vec<f32> = softmax(output, 1)
        .reshape([-1])
        .into_data()
        .as_slice()
        .unwrap()
        .to_vec();

    let mut vec = zip(
        vec![
            "GIMP".to_string(),
            "Firefox".to_string(),
            "Audacity".to_string(),
            "Chrome".to_string(),
            "VLC".to_string(),
        ],
        output,
    )
    .collect::<Vec<_>>();
    vec.sort_unstable_by(|(_, a), (_, b)| {
        if a < b {
            std::cmp::Ordering::Less
        } else if a > b {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });

    for (i, j) in vec {
        println!("{i} {j}");
    }
}
