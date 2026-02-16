import math
import random

import torch
import torch.nn as nn
from torch.nn.attention import SDPBackend, sdpa_kernel
import torch.nn.functional as F

# 1. SETUP DEVICE & SILENCE PROTOTYPE WARNINGS
device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
print(f"Using device: {device}")


# 2. VOCABULARY & POSITIONAL ENCODING
class Vocab:
    def __init__(self, data):
        self.stoi = {"<PAD>": 0, "<UNK>": 1}
        for list_of_strs in data:
            for s in list_of_strs:
                for word in s.lower().split():
                    if word not in self.stoi:
                        self.stoi[word] = len(self.stoi)

    def encode(self, text, max_len):
        tokens = [self.stoi.get(w, 1) for w in text.lower().split()][:max_len]
        return tokens + [0] * (max_len - len(tokens))


class PositionalEncoding(nn.Module):
    def __init__(self, d_model, max_len=5000):
        super().__init__()
        pe = torch.zeros(max_len, d_model)
        position = torch.arange(0, max_len, dtype=torch.float).unsqueeze(1)
        div_term = torch.exp(
            torch.arange(0, d_model, 2).float() * (-math.log(10000.0) / d_model)
        )
        pe[:, 0::2] = torch.sin(position * div_term)
        pe[:, 1::2] = torch.cos(position * div_term)
        self.register_buffer("pe", pe.unsqueeze(0))

    def forward(self, x):
        return x + self.pe[:, : x.size(1)]


# 3. RANKER MODEL
class TransformerRanker(nn.Module):
    def __init__(self, vocab_size, d_model=64, nhead=4, num_layers=2):
        super().__init__()
        self.d_model = d_model
        self.embedding = nn.Embedding(vocab_size, d_model, padding_idx=0)
        self.pos_encoder = PositionalEncoding(d_model)

        # Level 1: String Encoder
        enc_layer1 = nn.TransformerEncoderLayer(
            d_model=d_model, nhead=nhead, batch_first=True
        )
        # enable_nested_tensor=False prevents the NestedTensor prototype warning
        self.string_transformer = nn.TransformerEncoder(
            enc_layer1, num_layers=num_layers, enable_nested_tensor=False
        )

        # Level 2: List Interaction Encoder
        enc_layer2 = nn.TransformerEncoderLayer(
            d_model=d_model, nhead=nhead, batch_first=True
        )
        self.list_transformer = nn.TransformerEncoder(
            enc_layer2, num_layers=num_layers, enable_nested_tensor=False
        )

        self.scorer = nn.Linear(d_model, 1)

    def forward(self, x, word_mask, list_mask):
        # sdpa_kernel(SDPBackend.MATH) forces the stable math implementation.
        # This prevents experimental AMD Flash Attention kernels from triggering warnings.
        with sdpa_kernel(SDPBackend.MATH):
            batch_size, list_len, seq_len = x.shape
            x_flat = x.view(-1, seq_len)
            word_mask_flat = word_mask.view(-1, seq_len)

            # Step 1: Word Embeddings -> String Context
            feat = self.embedding(x_flat) * math.sqrt(self.d_model)
            feat = self.pos_encoder(feat)
            string_out = self.string_transformer(
                feat, src_key_padding_mask=word_mask_flat
            )

            # Mean pooling to get 1 vector per string
            mask_float = (~word_mask_flat).float().unsqueeze(-1)
            string_vecs = (string_out * mask_float).sum(dim=1) / mask_float.sum(
                dim=1
            ).clamp(min=1e-9)

            # Step 2: List Context (Strings seeing other strings)
            list_input = string_vecs.view(batch_size, list_len, self.d_model)
            contextual_vecs = self.list_transformer(
                list_input, src_key_padding_mask=list_mask
            )

            # Step 3: Raw scores
            return self.scorer(contextual_vecs).squeeze(-1)


# 4. DATA HELPER
def prepare_batch(data_lists, label_lists, vocab, device, max_str_len=10):
    batch_size = len(data_lists)
    max_list_len = max(len(ln) for ln in data_lists)
    inputs = torch.zeros((batch_size, max_list_len, max_str_len), dtype=torch.long)
    labels = torch.zeros((batch_size, max_list_len))
    list_mask = torch.ones((batch_size, max_list_len), dtype=torch.bool)
    word_mask = torch.ones((batch_size, max_list_len, max_str_len), dtype=torch.bool)

    for i, (str_list, lbl_list) in enumerate(zip(data_lists, label_lists)):
        for j, (s, lb) in enumerate(zip(str_list, lbl_list)):
            tokens = vocab.encode(s, max_str_len)
            inputs[i, j] = torch.tensor(tokens)
            labels[i, j] = lb
            list_mask[i, j] = False
            for k, tok in enumerate(tokens):
                if tok != 0:
                    word_mask[i, j, k] = False
    return (
        inputs.to(device),
        labels.to(device),
        word_mask.to(device),
        list_mask.to(device),
    )


# --- EXECUTION ---

train_data = []
for _ in range(20):
    full_sequence = list(range(1, 11))

    # 2. Decide how many elements to remove (1 to 9)
    # This means we will keep between 1 and 9 elements.
    num_to_keep = 10 - random.randint(1, 9)

    # 3. Randomly select the elements to keep
    # We use sorted() to ensure they stay in the original numerical order
    modified_list = sorted(random.sample(full_sequence, num_to_keep))

    # 4. Create a list of the indexes of the modified_list
    # In Python, indexes start at 0.
    indices = list(range(len(modified_list)))
    train_data.append((list(map(str, modified_list)), indices))

vocab = Vocab([d[0] for d in train_data])
model = TransformerRanker(len(vocab.stoi)).to(device)
optimizer = torch.optim.Adam(model.parameters(), lr=0.001)

model.train()
for epoch in range(200):
    optimizer.zero_grad()
    texts, targets = zip(*train_data)
    x, y, w_mask, l_mask = prepare_batch(texts, targets, vocab, device)

    logits = model(x, w_mask.view(-1, x.shape[-1]), l_mask)
    logits = logits.masked_fill(l_mask, -1e9)

    # ListNet Loss
    targets_soft = F.softmax(y.masked_fill(l_mask, -1e9), dim=1)
    loss = -(targets_soft * F.log_softmax(logits, dim=1)).sum(dim=1).mean()

    loss.backward()
    optimizer.step()
    if (epoch + 1) % 20 == 0:
        print(f"Epoch {epoch + 1} | Loss: {loss.item():.4f}")

model.eval()
test_list = list(map(str, list(range(10, 0, -1))))
with torch.no_grad():
    x_t, _, w_t, l_t = prepare_batch(
        [test_list], [[0 for _ in test_list]], vocab, device
    )
    scores = model(x_t, w_t.view(-1, x_t.shape[-1]), l_t)
    probs = torch.softmax(scores, dim=1)

print("\nResults:")
for s, p in sorted(zip(test_list, probs[0]), key=lambda tuple: tuple[1].item()):
    print(f"Importance: {p.item():.4f} | String: {s}")
