# Squig Model Server

> Standalone desktop application for running local LLMs — similar to LM Studio. Single binary, cross-platform (Linux & Windows), built-in model management, and an OpenAI-compatible API.

## Architecture

```
┌──────────────────────────────────────────────────┐
│  Tauri 2 Desktop Shell (native window)           │
├──────────────────────────────────────────────────┤
│  Svelte Dashboard UI (embedded in binary)        │
│  Model management, chat, hardware monitor        │
├──────────────────────────────────────────────────┤
│  Rust / Axum API Server                          │
│  OpenAI-compatible REST, SSE streaming           │
│  Multi-model routing, auth, metrics              │
├──────────────────────────────────────────────────┤
│  Inference Manager                               │
│  Manages llama-server sidecar processes          │
│  One llama-server per loaded model               │
│  Continuous batching, speculative decoding        │
├──────────────────────────────────────────────────┤
│  Model Registry                                  │
│  Scans directories for GGUF files                │
│  Auto-detects model family, params, quantization │
├──────────────────────────────────────────────────┤
│  Hardware Detection                              │
│  Auto-detects GPU, recommends backend            │
│  Vulkan / CUDA / ROCm / CPU                      │
└──────────────────────────────────────────────────┘
```

## Prerequisites

1. **Rust** (1.75+): https://rustup.rs
2. **Node.js** (20+): https://nodejs.org
3. **llama.cpp** (built with your GPU backend):

```bash
# Build llama.cpp with Vulkan (AMD iGPU / Strix Halo)
git clone https://github.com/ggml-org/llama.cpp
cd llama.cpp
cmake -B build -DGGML_VULKAN=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build -j$(nproc)
sudo cp build/bin/llama-server /usr/local/bin/

# Or with CUDA (NVIDIA)
cmake -B build -DGGML_CUDA=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build -j$(nproc)
```

## Quick Start

```bash
# 1. Build everything (UI + server)
chmod +x scripts/build-linux.sh
./scripts/build-linux.sh

# 2. Place GGUF models in ~/.squig-models/
mkdir -p ~/.squig-models
# Download a model (example):
# huggingface-cli download Qwen/Qwen2.5-Coder-32B-Instruct-GGUF \
#   qwen2.5-coder-32b-instruct-q5_k_m.gguf --local-dir ~/.squig-models

# 3. Run the server
./target/release/squig-model-server

# The desktop app opens automatically.
# API available at  http://127.0.0.1:9090/v1
```

### Windows

```powershell
# Build
.\scripts\build-windows.ps1

# Run
.\target\release\squig-model-server.exe
```

## Configuration

Edit `config.toml` (auto-generated on first run):

```toml
[server]
host = "127.0.0.1"
port = 9090

[models]
directories = ["~/.squig-models"]
default_model = "qwen2.5-coder-32b"   # auto-load on startup
max_loaded_models = 2

[inference]
gpu_layers = -1          # -1 = offload all to GPU
context_size = 32768
parallel_slots = 4       # concurrent request slots
flash_attention = true
gpu_backend = "auto"     # auto, vulkan, cuda, rocm, cpu
kv_cache_type = "q8_0"   # KV cache quantization

[inference.speculative]
enabled = true
draft_model = "/path/to/small-draft-model.gguf"
draft_max = 16
draft_min = 4
```

## API Usage

Fully OpenAI-compatible — works with any OpenAI client library.

```bash
# Chat completions
curl http://127.0.0.1:9090/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen2.5-coder-32b-instruct-q5_k_m",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }'

# List models
curl http://127.0.0.1:9090/v1/models

# Server status
curl http://127.0.0.1:9090/api/status

# Load a model via API
curl -X POST http://127.0.0.1:9090/api/models/load \
  -H "Content-Type: application/json" \
  -d '{"model": "qwen2.5-coder-32b"}'
```

### Use with Python (OpenAI SDK)

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:9090/v1",
    api_key="not-needed",
)

response = client.chat.completions.create(
    model="qwen2.5-coder-32b-instruct-q5_k_m",
    messages=[{"role": "user", "content": "Write a fibonacci function in Rust"}],
    stream=True,
)
for chunk in response:
    print(chunk.choices[0].delta.content or "", end="")
```

## Key Features

| Feature                     | Description                                             |
| --------------------------- | ------------------------------------------------------- |
| **Standalone desktop app**  | Native window via Tauri 2, UI embedded via `rust-embed` |
| **Cross-platform**          | Compiles + runs on Linux and Windows                    |
| **OpenAI-compatible**       | Drop-in replacement for OpenAI API                      |
| **Multi-model**             | Load/unload multiple models simultaneously              |
| **Continuous batching**     | Serve N concurrent requests per model                   |
| **Speculative decoding**    | 2-3x faster with draft model acceleration               |
| **Flash attention**         | Reduced KV cache memory usage                           |
| **KV cache quantization**   | Further memory savings (q8_0, q4_0)                     |
| **Auto hardware detection** | Detects GPU and recommends optimal backend              |
| **Built-in dashboard**      | Real-time metrics, model management, built-in chat      |
| **SSE streaming**           | Token-by-token streaming responses                      |

## Optimal Settings for Ryzen AI Max+ 395 (128GB)

```toml
[inference]
gpu_layers = -1           # Offload everything to iGPU
context_size = 65536      # Can afford large contexts with 128GB unified mem
parallel_slots = 8        # 8 concurrent requests
flash_attention = true
gpu_backend = "vulkan"    # Best iGPU support currently
kv_cache_type = "q8_0"

[inference.speculative]
enabled = true
draft_model = "~/.squig-models/qwen2.5-coder-0.5b-instruct-q8_0.gguf"
draft_max = 16
draft_min = 4
```

## Development

```bash
# Terminal 1: Run Rust server (recompiles on change with cargo-watch)
cargo install cargo-watch
cargo watch -x run

# Terminal 2: Run UI dev server (hot reload)
cd ui
npm install
npm run dev
# Open http://localhost:5173 (proxies API to :9090)
```

## Project Structure

```
squig-model-server/
├── Cargo.toml              # Rust library + CLI binary
├── config.toml             # Server configuration
├── src-tauri/              # Tauri 2 desktop wrapper
│   ├── Cargo.toml
│   ├── tauri.conf.json     # Window config, app metadata
│   ├── src/
│   │   ├── lib.rs          # Spawns Axum backend, configures Tauri
│   │   └── main.rs         # Windows subsystem entry
│   └── splash/
│       └── index.html      # Loading screen while backend starts
├── src/
│   ├── main.rs             # Entry point, CLI args
│   ├── config.rs           # Configuration loading/defaults
│   ├── server.rs           # Axum server setup, app state
│   ├── api/
│   │   ├── mod.rs          # Route definitions
│   │   ├── chat.rs         # POST /v1/chat/completions
│   │   ├── completions.rs  # POST /v1/completions
│   │   ├── models.rs       # GET /v1/models
│   │   ├── health.rs       # GET /api/health
│   │   └── management.rs   # Model load/unload, metrics, status
│   ├── inference/
│   │   ├── mod.rs
│   │   ├── engine.rs       # llama-server process management
│   │   ├── hardware.rs     # GPU/CPU detection
│   │   └── types.rs        # Metrics types
│   ├── models/
│   │   ├── mod.rs
│   │   └── registry.rs     # GGUF file discovery & metadata parsing
│   └── ui/
│       └── mod.rs          # Embedded static file serving
├── ui/                     # Svelte 5 dashboard
│   ├── package.json
│   ├── vite.config.js
│   ├── index.html
│   └── src/
│       ├── main.js
│       ├── App.svelte      # Main dashboard layout
│       ├── lib/api.js      # API client
│       └── components/
│           ├── ModelCard.svelte
│           ├── HardwarePanel.svelte
│           ├── MetricsPanel.svelte
│           └── ChatPanel.svelte
└── scripts/
    ├── build-linux.sh
    └── build-windows.ps1
```

## License

MIT — Copyright (c) 2026 Squig-AI
