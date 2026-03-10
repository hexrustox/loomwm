use std::{fs::create_dir_all, path::PathBuf};

use burn::{
    config::Config,
    data::{dataloader::DataLoaderBuilder, dataset::Dataset},
    module::Module,
    optim::{AdamConfig, Optimizer, decay::WeightDecayConfig},
    prelude::Backend,
    record::CompactRecorder,
    tensor::{ElementConversion, Float, Tensor, activation::log_sigmoid, backend::AutodiffBackend},
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
            weights,
        }: RankingBatch<B>,
    ) -> RegressionOutput<B> {
        let output = self.forward(inputs, word_mask, list_mask.clone());
        let output = output.mask_fill(list_mask.clone(), -1e9);

        // 1. Get dimensions for reshaping
        let [batch_size, list_size] = output.dims();

        // 2. Prepare Scores and Targets for Broadcasting
        // Use reshape to get [Batch, List, 1] and [Batch, 1, List]
        let s_i = output.clone().reshape([batch_size, list_size, 1]);
        let s_j = output.clone().reshape([batch_size, 1, list_size]);
        let s_diff = s_i - s_j; // Shape: [Batch, List, List]

        // targets is already a Tensor, use directly.
        // If targets were Int, you would use: targets.into_float()
        let t_i = targets.clone().reshape([batch_size, list_size, 1]);
        let t_j = targets.clone().reshape([batch_size, 1, list_size]);
        let t_diff = t_i - t_j;

        // 3. Create Masks
        // list_mask is True for Padding -> Not Padding is False.
        // We need a mask that is True for Valid items.
        // We use bool_not() to invert: True (Padding) -> False (Valid)
        let is_valid_i = list_mask
            .clone()
            .bool_not()
            .reshape([batch_size, list_size, 1]);
        let is_valid_j = list_mask.bool_not().reshape([batch_size, 1, list_size]);

        // valid_pair_mask is True where BOTH items are valid (not padded)
        let valid_pair_mask = is_valid_i.bool_and(is_valid_j);

        // Identify pairs where target_i > target_j
        let target_mask = t_diff.greater_elem(0.0);

        // 4. Determine Active Pairs
        // A pair is active if it is valid AND target_i > target_j
        let active_pairs = valid_pair_mask.bool_and(target_mask);

        // 5. Compute Pairwise Logistic Loss
        // Loss = log(1 + exp(-(s_i - s_j)))
        let pair_loss = log_sigmoid(s_diff).neg();

        // Mask out inactive pairs (set loss to 0.0)
        // active_pairs is a Bool tensor, so we negate it to mask the unwanted positions
        let loss_masked = pair_loss.mask_fill(active_pairs.clone().bool_not(), 0.0);

        // weights shape: [batch_size]
        // Reshape to [batch_size, 1, 1] to broadcast over the pairs [batch_size, list_size, list_size]
        let weights_reshaped = weights.reshape([batch_size, 1, 1]);

        // Apply weights to the loss
        // This multiplies every pair in batch 'b' by weight 'w_b'
        let weighted_loss = loss_masked * weights_reshaped;

        // 6. Normalize by the number of active pairs
        let num_active = active_pairs.float().sum();

        let loss = weighted_loss.sum() / (num_active + 1e-9);

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
    #[config(default = 0.5)]
    pub old_data_weight: f32,
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

    let batcher = RankingBatcher::new(vocab).with_weight(config.old_data_weight);

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
