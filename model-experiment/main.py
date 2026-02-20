import math
import random

import torch
import torch.nn as nn
from torch.nn.attention import SDPBackend, sdpa_kernel
import torch.nn.functional as F
from torch.utils.data import DataLoader, Dataset

# 1. SETUP DEVICE
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
    def __init__(self, vocab_size, d_model, nhead, num_layers, dropout):
        super().__init__()
        self.d_model = d_model
        self.embedding = nn.Embedding(vocab_size, d_model, padding_idx=0)
        self.pos_encoder = PositionalEncoding(d_model)
        self.dropout = nn.Dropout(p=dropout)

        enc_layer1 = nn.TransformerEncoderLayer(
            d_model=d_model, nhead=nhead, dropout=dropout, batch_first=True
        )
        self.string_transformer = nn.TransformerEncoder(
            enc_layer1, num_layers=num_layers, enable_nested_tensor=False
        )

        enc_layer2 = nn.TransformerEncoderLayer(
            d_model=d_model, nhead=nhead, dropout=dropout, batch_first=True
        )
        self.list_transformer = nn.TransformerEncoder(
            enc_layer2, num_layers=num_layers, enable_nested_tensor=False
        )

        self.scorer = nn.Linear(d_model, 1)

    def forward(self, x, word_mask, list_mask):
        with sdpa_kernel(SDPBackend.MATH):
            batch_size, list_len, seq_len = x.shape
            x_flat = x.view(-1, seq_len)
            word_mask_flat = word_mask.view(-1, seq_len)

            feat = self.embedding(x_flat) * math.sqrt(self.d_model)
            feat = self.pos_encoder(feat)
            feat = self.dropout(self.pos_encoder(feat))

            string_out = self.string_transformer(
                feat, src_key_padding_mask=word_mask_flat
            )

            mask_float = (~word_mask_flat).float().unsqueeze(-1)
            string_vecs = (string_out * mask_float).sum(dim=1) / mask_float.sum(
                dim=1
            ).clamp(min=1e-9)

            list_input = string_vecs.view(batch_size, list_len, self.d_model)
            contextual_vecs = self.list_transformer(
                list_input, src_key_padding_mask=list_mask
            )

            return self.scorer(contextual_vecs).squeeze(-1)


def prepare_inference_batch(data_lists, vocab, device, max_str_len=10):
    batch_size = len(data_lists)
    max_list_len = max(len(ln) for ln in data_lists)

    inputs = torch.zeros((batch_size, max_list_len, max_str_len), dtype=torch.long)
    list_mask = torch.ones((batch_size, max_list_len), dtype=torch.bool)
    word_mask = torch.ones((batch_size, max_list_len, max_str_len), dtype=torch.bool)

    for i, str_list in enumerate(data_lists):
        for j, s in enumerate(str_list):
            tokens = vocab.encode(s, max_str_len)
            inputs[i, j] = torch.tensor(tokens)
            list_mask[i, j] = False
            for k, tok in enumerate(tokens):
                if tok != 0:
                    word_mask[i, j, k] = False

    return (
        inputs.to(device),
        word_mask.to(device),
        list_mask.to(device),
    )


def generate_training_batch(base_samples, num_unknowns_range=(0, 2)):
    """Dynamically inject unknowns at random positions"""
    batch = []
    for str_list in base_samples:
        augmented = str_list.copy()
        num_unknowns = random.randint(*num_unknowns_range)

        for _ in range(num_unknowns):
            pos = random.randint(0, len(augmented))
            augmented.insert(pos, "<UNK>")

        batch.append(augmented)
    return batch


class RankingDataset(Dataset):
    def __init__(self, base_data):
        self.base_data = base_data

    def __len__(self):
        return len(self.base_data)

    def __getitem__(self, idx):
        return self.base_data[idx]


def collate_fn(batch, vocab, device, max_str_len=10, inject_unknowns=True):
    # Optionally inject unknowns during training
    if inject_unknowns:
        batch = generate_training_batch(batch, num_unknowns_range=(0, 2))

    batch_size = len(batch)
    max_list_len = max(len(str_list) for str_list in batch)

    inputs = torch.zeros((batch_size, max_list_len, max_str_len), dtype=torch.long)
    labels = torch.zeros((batch_size, max_list_len))
    list_mask = torch.ones((batch_size, max_list_len), dtype=torch.bool)
    word_mask = torch.ones((batch_size, max_list_len, max_str_len), dtype=torch.bool)

    for i, str_list in enumerate(batch):
        known_items = [(j, s) for j, s in enumerate(str_list) if s != "<UNK>"]
        unknown_items = [(j, s) for j, s in enumerate(str_list) if s == "<UNK>"]

        rank = 0
        item_ranks = {}

        for j, s in known_items:
            item_ranks[j] = rank
            rank += 1

        for j, s in unknown_items:
            item_ranks[j] = rank
            rank += 1

        for j, s in enumerate(str_list):
            tokens = vocab.encode(s, max_str_len)
            inputs[i, j] = torch.tensor(tokens)
            labels[i, j] = item_ranks[j]
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

app_samples = [
    ["Firefox", "VLC", "GIMP"],
    ["Chrome", "LibreOffice", "Nautilus", "Blender"],
    ["Firefox", "Chrome"],
    ["VLC", "GIMP", "Thunderbird", "Audacity", "Inkscape"],
    ["Firefox", "LibreOffice", "Audacity"],
    ["Chrome", "VLC", "Nautilus", "Blender"],
    ["Firefox", "GIMP", "Inkscape"],
    ["LibreOffice", "VLC", "Thunderbird", "Audacity"],
    ["Firefox", "Chrome", "Nautilus", "Blender"],
    ["VLC", "Audacity", "Inkscape"],
]

test_example = ["Never Seen", "GIMP", "Firefox", "Audacity", "Chrome", "VLC"]
# expect: ["Firefox", "Chrome", "VLC", "GIMP", "Audacity", "Never Seen"]


# Build vocab and model
vocab = Vocab(app_samples)
model = TransformerRanker(
    len(vocab.stoi), d_model=32, nhead=4, num_layers=1, dropout=0.2
).to(device)
optimizer = torch.optim.Adam(model.parameters(), lr=0.002, weight_decay=1e-4)

# DataLoader
dataset = RankingDataset(app_samples)
loader = DataLoader(
    dataset,
    batch_size=4,
    shuffle=True,
    collate_fn=lambda batch: collate_fn(batch, vocab, device, inject_unknowns=True),
)

# Training
model.train()
for epoch in range(100):
    epoch_loss = 0.0
    num_batches = 0

    for x, y, w_mask, l_mask in loader:
        optimizer.zero_grad()

        logits = model(x, w_mask, l_mask)
        logits = logits.masked_fill(l_mask, -1e9)

        targets_soft = F.softmax(y.masked_fill(l_mask, -1e9), dim=1)
        loss = -(targets_soft * F.log_softmax(logits, dim=1)).sum(dim=1).mean()

        loss.backward()
        optimizer.step()

        epoch_loss += loss.item()
        num_batches += 1

    if (epoch + 1) % 20 == 0:
        avg_loss = epoch_loss / num_batches
        print(f"Epoch {epoch + 1} | Avg Loss: {avg_loss:.4f}")

# Test with unknowns
model.eval()
with torch.no_grad():
    x_t, w_t, l_t = prepare_inference_batch([test_example], vocab, device)
    scores = model(x_t, w_t, l_t)
    probs = torch.softmax(scores, dim=1)

print("\nResults")
for s, p in sorted(zip(test_example, probs[0]), key=lambda t: t[1].item()):
    print(f"Score: {p.item():.4f} | {s}")
