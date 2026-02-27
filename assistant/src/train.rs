use std::fs::create_dir_all;

use burn::{
    config::Config,
    data::{dataloader::DataLoaderBuilder, dataset::Dataset},
    module::Module,
    optim::{AdamConfig, Optimizer},
    prelude::Backend,
    record::CompactRecorder,
    tensor::{
        ElementConversion,
        activation::{log_softmax, softmax},
        backend::AutodiffBackend,
    },
    train::{InferenceStep, RegressionOutput, TrainOutput, TrainStep},
};

use crate::{
    data::{RankingBatch, RankingBatcher, RankingDataset, Vocab},
    model::{RankerModel, RankerModelConfig},
};

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
    #[config(default = 4)]
    pub batch_size: usize,
    #[config(default = 4)]
    pub num_workers: usize,
    #[config(default = 42)]
    pub seed: u64,
    #[config(default = 0.001)]
    pub learning_rate: f64,
}

pub fn train<B: AutodiffBackend>(
    artifact_dir: &str,
    training_dataset: RankingDataset,
    device: B::Device,
) {
    create_dir_all(artifact_dir).unwrap();

    let vocab = Vocab::new(training_dataset.iter().flat_map(|item| item.app_ids));
    std::fs::write(
        format!("{artifact_dir}/vocab.json"),
        serde_json::to_string(&vocab).unwrap(),
    )
    .unwrap();

    let config = TrainingConfig::new(
        RankerModelConfig::new(vocab.vocab_size()),
        AdamConfig::new(),
    );
    config.save(format!("{artifact_dir}/config.json")).unwrap();

    B::seed(&device, config.seed);

    let batcher = RankingBatcher::new(vocab);

    let dataloader_train = DataLoaderBuilder::new(batcher)
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        .num_workers(config.num_workers)
        .build(training_dataset);

    let mut model = config.model.init::<B>(&device);
    let mut optimizer = config.optimizer.init();

    let mut best_loss = None;
    let mut tolerance = 0.05;
    let tolerance_step = 0.002;
    let min_tolerance: f32 = 0.001;
    let mut patience_counter = 0;
    let max_patience = 3;
    loop {
        let mut loss = 0.0;
        let mut num_batches = 0.0;
        for batch in dataloader_train.iter() {
            let output = TrainStep::step(&model, batch);
            let grads = output.grads;
            model = optimizer.step(config.learning_rate, model, grads);

            loss += output.item.loss.into_scalar().elem::<f32>();
            num_batches += 1.0;
        }

        let avg_loss = loss / num_batches;

        if let Some(loss) = best_loss {
            if avg_loss < loss {
                patience_counter = 0;
                best_loss = Some(avg_loss);
            } else if avg_loss > loss * (1.0 + tolerance) {
                patience_counter += 1;
            } else {
                continue;
            }
        } else {
            best_loss = Some(avg_loss);
        }

        if patience_counter >= max_patience {
            break;
        }

        tolerance = min_tolerance.max(tolerance - tolerance_step);
    }

    model
        .save_file(format!("{artifact_dir}/model"), &CompactRecorder::new())
        .unwrap();
}
