use burn::{
    config::Config,
    data::{dataloader::batcher::Batcher, dataset::Dataset},
    module::Module,
    prelude::Backend,
    record::{CompactRecorder, Recorder},
    tensor::activation::softmax,
};

use crate::{
    RankingDataset, RankingItem,
    data::RankingBatcher,
    train::{TrainingConfig, Vocab},
};

pub fn infer<B: Backend>(
    artifact_dir: &str,
    training_dataset: RankingDataset,
    testing_dataset: RankingDataset,
    device: B::Device,
) {
    let config = TrainingConfig::load(format!("{artifact_dir}/config.json"))
        .expect("Config should exist for the model; run train first");
    let record = CompactRecorder::new()
        .load(format!("{artifact_dir}/model").into(), &device)
        .expect("Trained model should exist; run train first");

    let model = config.model.init::<B>(&device).load_record(record);

    let vocab = Vocab::new(training_dataset.iter().flat_map(|item| item.app_ids));
    let batcher = RankingBatcher::new(vocab);
    let batch: crate::data::RankingBatch<B> =
        batcher.batch(testing_dataset.iter().collect(), &device);
    let output = model.forward(batch.inputs, batch.word_mask, batch.list_mask);
    let output: Vec<Vec<f32>> = softmax(output, 1)
        .into_data()
        .to_vec()
        .unwrap()
        .chunks(5)
        .map(|a| a.to_vec())
        .collect::<Vec<_>>();

    for (RankingItem { app_ids }, labels) in testing_dataset.iter().zip(output.iter()) {
        let mut ls = app_ids.iter().zip(labels).collect::<Vec<_>>();
        ls.sort_by(|(_, a), (_, b)| {
            if a < b {
                std::cmp::Ordering::Less
            } else if a > b {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        println!("===");
        for (i, j) in ls {
            println!("{i}: {j}");
        }
    }
}
