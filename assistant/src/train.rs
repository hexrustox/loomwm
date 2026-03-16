use std::{fs::create_dir_all, path::PathBuf};

use burn::{
    config::Config,
    data::{dataloader::DataLoaderBuilder, dataset::Dataset},
    module::Module,
    optim::{AdamConfig, Optimizer, decay::WeightDecayConfig},
    prelude::Backend,
    record::CompactRecorder,
    tensor::{ElementConversion, Float, Tensor, backend::AutodiffBackend},
    train::{RegressionOutput, TrainOutput, TrainStep},
};
use tracing::{error, info};

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

        let masked_output = output.clone().mask_fill(list_mask.clone(), -1e9);

        let exp_scores = masked_output.clone().exp();

        let rev_exp = exp_scores.flip([1]);
        let rev_cum_sum = rev_exp.cumsum(1);
        let suffix_sums = rev_cum_sum.flip([1]);

        let log_probs = masked_output - suffix_sums.log();

        let valid_log_probs = log_probs.mask_fill(list_mask, 0.0);

        let list_loss = -valid_log_probs.sum_dim(1);
        let loss = list_loss.mean();

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
    #[config(default = 1e-4)]
    pub weight_decay: f32,
    #[config(default = 0.05)]
    pub tolerance: f32,
    #[config(default = 0.002)]
    pub tolerance_step: f32,
    #[config(default = 0.001)]
    pub min_tolerance: f32,
    #[config(default = 5)]
    pub max_patience: i32,
    #[config(default = 100)]
    pub max_epoch: i32,
}

// TODO pause training for inference, continuous learning?
pub fn train<B: AutodiffBackend>(
    model_dir: PathBuf,
    training_dataset: RankingDataset,
    device: B::Device,
) {
    info!("Start training");

    let vocab = Vocab::new(
        training_dataset
            .iter()
            .flat_map(|item| item.app_ids.clone()),
    );
    let vocab_json = serde_json::to_string(&vocab);

    let config = TrainingConfig::new(
        RankerModelConfig::new(vocab.vocab_size()),
        AdamConfig::new(),
    );

    B::seed(&device, config.seed);

    let batcher = RankingBatcher::new(vocab);

    let dataloader_train = DataLoaderBuilder::new(batcher)
        .batch_size(config.batch_size)
        .shuffle(config.seed)
        .num_workers(config.num_workers)
        .build(training_dataset);

    let mut model = config.model.init::<B>(&device);
    let mut optimizer = config
        .optimizer
        .clone()
        .with_weight_decay(Some(WeightDecayConfig::new(config.weight_decay)))
        .init();

    let mut best_loss = None;
    let mut tolerance = config.tolerance;
    let tolerance_step = config.tolerance_step;
    let min_tolerance: f32 = config.min_tolerance;
    let mut patience_counter = 0;
    let max_patience = config.max_patience;
    let mut loss_sum: Option<Tensor<B, 1, Float>> = None;
    let mut epoch = 0;
    let max_epoch = config.max_epoch;

    loop {
        if epoch >= max_epoch {
            break;
        } else {
            epoch += 1;
        }

        for batch in dataloader_train.iter() {
            let output = TrainStep::step(&model, batch);
            let grads = output.grads;
            model = optimizer.step(config.learning_rate, model, grads);

            let batch_loss = output.item.loss.clone();
            loss_sum = Some(match loss_sum {
                None => batch_loss,
                Some(acc) => acc + batch_loss,
            });
        }

        let avg_loss = loss_sum
            .take()
            .map(|t| t.into_scalar().elem::<f32>())
            .unwrap_or(0.0)
            / config.batch_size as f32;

        #[cfg(test)]
        println!("epoch: {epoch}, loss: {avg_loss}");

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

    let _ = create_dir_all(&model_dir);

    if let Err(e) =
        vocab_json.map(|contents| std::fs::write(model_dir.join("vocab.json"), contents))
    {
        error!("Failed to save model vocab: {e}");
    }

    if let Err(e) = config.save(model_dir.join("config.json")) {
        error!("Failed to save model config: {e}");
    }

    if let Err(e) = model.save_file(model_dir.join("model"), &CompactRecorder::new()) {
        error!("Failed to save model: {e}");
    }

    info!("Training completed")
}
