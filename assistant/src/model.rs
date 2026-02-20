use burn::module::Module;
use burn::nn::transformer::{
    TransformerEncoder, TransformerEncoderConfig, TransformerEncoderInput,
};
use burn::nn::{Dropout, DropoutConfig, Embedding, EmbeddingConfig, Linear, LinearConfig};
use burn::prelude::*;

// ─── Positional Encoding ───────────────────────────────────────────────────────

#[derive(Module, Debug)]
pub struct PositionalEncoding<B: Backend> {
    /// Pre-computed positional encoding table: shape [1, max_len, d_model]
    pe: Tensor<B, 3>,
}

impl<B: Backend> PositionalEncoding<B> {
    pub fn new(d_model: usize, max_len: usize, device: &B::Device) -> Self {
        let half = d_model / 2;

        // Build position values [0.0, 1.0, ..., max_len-1] as a Vec<f32>
        let position_data: Vec<f32> = (0..max_len).map(|i| i as f32).collect();
        let position =
            Tensor::<B, 1>::from_floats(position_data.as_slice(), device).reshape([max_len, 1]); // [max_len, 1]

        // Build div_term: exp(i * (-ln(10000) / d_model)) for i in 0..half
        let log_denom = -(10000.0_f32.ln() / d_model as f32);
        let div_data: Vec<f32> = (0..half).map(|i| (i as f32 * log_denom).exp()).collect();
        let div_term = Tensor::<B, 1>::from_floats(div_data.as_slice(), device).reshape([1, half]); // [1, half]

        // angles: [max_len, half]
        let angles = position.matmul(div_term);

        let sin_vals = angles.clone().sin(); // [max_len, half]
        let cos_vals = angles.cos(); // [max_len, half]

        // Interleave: stack [sin, cos] on a trailing dim then flatten back
        let sin_expanded = sin_vals.reshape([max_len, half, 1]);
        let cos_expanded = cos_vals.reshape([max_len, half, 1]);
        let interleaved = Tensor::cat(vec![sin_expanded, cos_expanded], 2);
        // [max_len, half, 2]

        let pe = interleaved.reshape([1, max_len, d_model]);

        Self { pe }
    }

    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let seq_len = x.dims()[1];
        let pe_slice = self.pe.clone().slice([0..1, 0..seq_len]);
        x + pe_slice
    }
}

// ─── Transformer Ranker ────────────────────────────────────────────────────────

#[derive(Config, Debug)]
pub struct RankerModelConfig {
    pub vocab_size: usize,
    #[config(default = 32)]
    pub d_model: usize,
    #[config(default = 4)]
    pub n_head: usize,
    #[config(default = 1)]
    pub num_layers: usize,
    #[config(default = 0.2)]
    pub dropout: f64,
}

#[derive(Module, Debug)]
pub struct RankerModel<B: Backend> {
    d_model: usize,
    embedding: Embedding<B>,
    pos_encoder: PositionalEncoding<B>,
    dropout: Dropout,
    string_transformer: TransformerEncoder<B>,
    list_transformer: TransformerEncoder<B>,
    scorer: Linear<B>,
}

impl RankerModelConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> RankerModel<B> {
        let embedding = EmbeddingConfig::new(self.vocab_size, self.d_model).init(device);
        let pos_encoder = PositionalEncoding::new(self.d_model, 5000, device);
        let dropout = DropoutConfig::new(self.dropout).init();

        let string_transformer = TransformerEncoderConfig::new(
            self.d_model,
            self.d_model * 4,
            self.n_head,
            self.num_layers,
        )
        .with_dropout(self.dropout)
        .init(device);

        let list_transformer = TransformerEncoderConfig::new(
            self.d_model,
            self.d_model * 4,
            self.n_head,
            self.num_layers,
        )
        .with_dropout(self.dropout)
        .init(device);

        let scorer = LinearConfig::new(self.d_model, 1).init(device);

        RankerModel {
            d_model: self.d_model,
            embedding,
            pos_encoder,
            dropout,
            string_transformer,
            list_transformer,
            scorer,
        }
    }
}

impl<B: Backend> RankerModel<B> {
    /// Forward pass.
    ///
    /// # Arguments
    /// * `x`         – Token IDs: `[batch_size, list_len, seq_len]` (Int tensor)
    /// * `word_mask` – Padding mask per token: `[batch_size, list_len, seq_len]` (Bool tensor)
    ///   `true` = **padded / ignore** (PyTorch convention)
    /// * `list_mask` – Padding mask per list element: `[batch_size, list_len]` (Bool tensor)
    ///   `true` = **padded / ignore**
    ///
    /// # Returns
    /// Scores: `[batch_size, list_len]`
    pub fn forward(
        &self,
        x: Tensor<B, 3, Int>,
        word_mask: Tensor<B, 3, Bool>,
        list_mask: Tensor<B, 2, Bool>,
    ) -> Tensor<B, 2> {
        let [batch_size, list_len, seq_len] = x.dims();

        // Flatten list dimension into batch: [batch_size * list_len, seq_len]
        let x_flat: Tensor<B, 2, Int> = x.reshape([batch_size * list_len, seq_len]);
        let word_mask_flat: Tensor<B, 2, Bool> =
            word_mask.reshape([batch_size * list_len, seq_len]);

        // Embedding + scale + positional encoding + dropout
        let scale = (self.d_model as f64).sqrt();
        let feat: Tensor<B, 3> = self.embedding.forward(x_flat) * scale;
        let feat = self.pos_encoder.forward(feat);
        let feat = self.dropout.forward(feat);

        // String-level transformer
        // Burn mask_pad: true = valid (opposite of PyTorch src_key_padding_mask)
        let string_pad_mask = word_mask_flat.clone().bool_not();
        let string_input = TransformerEncoderInput::new(feat).mask_pad(string_pad_mask);
        let string_out = self.string_transformer.forward(string_input);
        // [B*L, S, D]

        // Mean-pool over non-padded tokens
        let mask_float: Tensor<B, 3> =
            word_mask_flat
                .bool_not()
                .float()
                .reshape([batch_size * list_len, seq_len, 1]);

        let summed = (string_out * mask_float.clone()).sum_dim(1); // [B*L, 1, D]
        let counts = mask_float.sum_dim(1).clamp_min(1e-9); // [B*L, 1, D]
        let string_vecs = (summed / counts).reshape([batch_size, list_len, self.d_model]);

        // List-level transformer
        let list_pad_mask = list_mask.bool_not();
        let list_input = TransformerEncoderInput::new(string_vecs).mask_pad(list_pad_mask);
        let contextual_vecs = self.list_transformer.forward(list_input);
        // [batch_size, list_len, d_model]

        // Score each element → squeeze last dim
        let scores: Tensor<B, 3> = self.scorer.forward(contextual_vecs);
        scores.reshape([batch_size, list_len])
    }
}
