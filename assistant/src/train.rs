use std::{fs::create_dir_all, path::PathBuf};

use burn::{
    config::Config,
    data::{dataloader::DataLoaderBuilder, dataset::Dataset},
    module::Module,
    optim::{AdamConfig, Optimizer, decay::WeightDecayConfig},
    prelude::Backend,
    record::CompactRecorder,
    tensor::{
        ElementConversion, Float, Tensor,
        activation::{log_softmax, softmax},
        backend::AutodiffBackend,
    },
    train::{InferenceStep, RegressionOutput, TrainOutput, TrainStep},
};
use tracing::error;

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

// TODO pause training for inference, continuous learning?
pub fn train<B: AutodiffBackend>(
    model_dir: PathBuf,
    training_dataset: RankingDataset,
    device: B::Device,
) {
    let vocab = Vocab::new(
        training_dataset
            .iter()
            .flat_map(|item| item.app_ids.clone()),
    );
    let vocab_json = serde_json::to_string(&vocab);

    let config = TrainingConfig::new(
        RankerModelConfig::new(vocab.vocab_size()),
        AdamConfig::new().with_weight_decay(Some(WeightDecayConfig::new(1e-4))),
    );

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
    let mut loss_sum: Option<Tensor<B, 1, Float>> = None;
    loop {
        let mut num_batches = 0.0;
        for batch in dataloader_train.iter() {
            let output = TrainStep::step(&model, batch);
            let grads = output.grads;
            model = optimizer.step(config.learning_rate, model, grads);

            let batch_loss = output.item.loss.clone();
            loss_sum = Some(match loss_sum {
                None => batch_loss,
                Some(acc) => acc + batch_loss,
            });
            num_batches += 1.0;
        }

        let avg_loss = loss_sum
            .take()
            .map(|t| t.into_scalar().elem::<f32>())
            .unwrap_or(0.0)
            / num_batches as f32;

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

    if let Some(parent) = model_dir.parent() {
        let _ = create_dir_all(parent);
    }

    if let Err(e) =
        vocab_json.map(|contents| std::fs::write(model_dir.join("vocab.json"), contents))
    {
        error!("failed to save model vocab: {e}");
    }

    if let Err(e) = config.save(model_dir.join("config.json")) {
        error!("failed to save model config: {e}");
    }

    if let Err(e) = model.save_file(model_dir.join("model"), &CompactRecorder::new()) {
        error!("failed to save model: {e}");
    }
}
