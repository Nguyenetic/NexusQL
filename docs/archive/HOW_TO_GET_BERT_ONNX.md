# 🤖 How to Get BERT in ONNX Format

## Option 1: Download Pre-Converted (Easiest) ⭐

**HuggingFace has pre-converted ONNX models:**

```bash
# Install HuggingFace CLI (optional)
pip install huggingface-hub

# Download optimized BERT ONNX model
huggingface-cli download optimum/bert-base-uncased --local-dir ./models/bert-onnx
```

**Or use these direct links:**
- Model: https://huggingface.co/optimum/bert-base-uncased
- Small model: https://huggingface.co/optimum/distilbert-base-uncased

## Option 2: Convert from PyTorch

```python
import torch
from transformers import BertModel, BertTokenizer

# Load model
model = BertModel.from_pretrained("bert-base-uncased")
tokenizer = BertTokenizer.from_pretrained("bert-base-uncased")

# Dummy input
dummy_input = torch.randint(0, 1000, (1, 128))

# Export to ONNX
torch.onnx.export(
    model,
    dummy_input,
    "bert-base-uncased.onnx",
    input_names=['input_ids'],
    output_names=['last_hidden_state'],
    dynamic_axes={
        'input_ids': {0: 'batch', 1: 'sequence'},
        'last_hidden_state': {0: 'batch', 1: 'sequence'}
    },
    opset_version=14
)

# Save tokenizer
tokenizer.save_pretrained("./")
```

## Option 3: Use Optimum Library (Best Quality)

```python
from optimum.onnxruntime import ORTModelForFeatureExtraction
from transformers import AutoTokenizer

# Load and convert automatically
model = ORTModelForFeatureExtraction.from_pretrained(
    "bert-base-uncased", 
    export=True  # Auto-convert to ONNX
)

tokenizer = AutoTokenizer.from_pretrained("bert-base-uncased")

# Save
model.save_pretrained("./models/bert-onnx")
tokenizer.save_pretrained("./models/bert-onnx")
```

## For NexusSQL:

**We'll use Option 1** - Download pre-converted models

**Models to get:**
1. `distilbert-base-uncased` (66M params, faster)
2. `bert-base-uncased` (110M params, more accurate)

**Files needed:**
- `model.onnx` (the neural network)
- `tokenizer.json` (text → tokens)

**Once ONNX compiles, we'll download and test!** 🚀
