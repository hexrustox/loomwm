use std::collections::HashMap;

use burn::{
    config::Config,
    data::{dataloader::DataLoaderBuilder, dataset::Dataset},
    module::Module,
    optim::AdamConfig,
    prelude::Backend,
    record::CompactRecorder,
    tensor::{
        activation::{log_softmax, softmax},
        backend::AutodiffBackend,
    },
    train::{
        InferenceStep, Learner, RegressionOutput, SupervisedTraining, TrainOutput, TrainStep,
        metric::LossMetric,
    },
};

use crate::{
    data::{RankingBatch, RankingBatcher, RankingDataset},
    model::{RankerModel, RankerModelConfig},
};

#[derive(Clone)]
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

impl<B: Backend> RankerModel<B> {
    pub fn forward_regression(
        &self,
        RankingBatch {
            inputs,
            targets,
            word_mask,
            list_mask,
        }: RankingBatch<B>,
    ) -> RegressionOutput<B> {
        let output = self.forward(inputs, word_mask, list_mask.clone());
        let output = output.mask_fill(list_mask.clone(), -1e9);

        let labels_soft = softmax(targets.clone().mask_fill(list_mask, -1e9), 1);
        let loss = -(labels_soft * log_softmax(output.clone(), 1))
            .sum_dim(1)
            .mean();

        RegressionOutput::new(loss, output, targets)
    }
}

impl<B: AutodiffBackend> TrainStep for RankerModel<B> {
    type Input = RankingBatch<B>;
    type Output = RegressionOutput<B>;

    fn step(&self, batch: Self::Input) -> TrainOutput<Self::Output> {
        let item = self.forward_regression(batch);

        TrainOutput::new(self, item.loss.backward(), item)
    }
}

impl<B: Backend> InferenceStep for RankerModel<B> {
    type Input = RankingBatch<B>;
    type Output = RegressionOutput<B>;

    fn step(&self, batch: Self::Input) -> RegressionOutput<B> {
        self.forward_regression(batch)
    }
}

#[derive(Config, Debug)]
pub struct TrainingConfig {
    pub model: RankerModelConfig,
    pub optimizer: AdamConfig,
    #[config(default = 100)]
    pub num_epochs: usize,
    #[config(default = 4)]
    pub batch_size: usize,
    #[config(default = 4)]
    pub num_workers: usize,
    #[config(default = 42)]
    pub seed: u64,
    #[config(default = 0.002)]
    pub learning_rate: f64,
}

fn create_artifact_dir(artifact_dir: &str) {
    // Remove existing artifacts before to get an accurate learner summary
    std::fs::remove_dir_all(artifact_dir).ok();
    std::fs::create_dir_all(artifact_dir).ok();
}

pub fn train<B: AutodiffBackend>(
    artifact_dir: &str,
    training_dataset: RankingDataset,
    testing_dataset: RankingDataset,
    device: B::Device,
) {
    create_artifact_dir(artifact_dir);

    let vocab = Vocab::new(training_dataset.iter().flat_map(|item| item.app_ids));

    let config = TrainingConfig::new(
        RankerModelConfig::new(vocab.vocab_size()),
        AdamConfig::new(),
    );
    config
        .save(format!("{artifact_dir}/config.json"))
        .expect("Config should be saved successfully");

    B::seed(&device, config.seed);

    let batcher = RankingBatcher::new(vocab);

    let dataloader_train = DataLoaderBuilder::new(batcher.clone())
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        .num_workers(config.num_workers)
        .build(training_dataset);

    let dataloader_test = DataLoaderBuilder::new(batcher)
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        .num_workers(config.num_workers)
        .build(testing_dataset);

    let training = SupervisedTraining::new(artifact_dir, dataloader_train, dataloader_test)
        .metric_train_numeric(LossMetric::new())
        .metric_valid_numeric(LossMetric::new())
        .with_file_checkpointer(CompactRecorder::new())
        .num_epochs(config.num_epochs)
        .summary();

    let model = config.model.init::<B>(&device);
    let result = training.launch(Learner::new(
        model,
        config.optimizer.init(),
        config.learning_rate,
    ));

    result
        .model
        .save_file(format!("{artifact_dir}/model"), &CompactRecorder::new())
        .expect("Trained model should be saved successfully");
}
