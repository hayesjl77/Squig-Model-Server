<script>
  let activeSection = $state('getting-started');

  const sections = [
    { id: 'getting-started', label: '🚀 Getting Started', icon: '🚀' },
    { id: 'models', label: '📦 Models', icon: '📦' },
    { id: 'settings', label: '⚙️ Settings', icon: '⚙️' },
    { id: 'api', label: '🔌 API & Server', icon: '🔌' },
    { id: 'tools', label: '🛠️ Tool Calling', icon: '🛠️' },
    { id: 'tips', label: '💡 Tips & Tricks', icon: '💡' },
    { id: 'about', label: '✦ About Squig AI', icon: '✦' },
  ];
</script>

<div class="help-panel">
  <div class="help-sidebar">
    <h2>📖 Guide</h2>
    <nav>
      {#each sections as section}
        <button
          class:active={activeSection === section.id}
          onclick={() => activeSection = section.id}
        >
          {section.label}
        </button>
      {/each}
    </nav>
  </div>

  <div class="help-content">

    {#if activeSection === 'getting-started'}
      <h2>Getting Started</h2>
      <p class="intro">Squig Model Server is a desktop app for running local LLMs with an OpenAI-compatible API. Below is everything you need to get up and running.</p>

      <div class="help-card">
        <h3>1. Add Models</h3>
        <p>Place <code>.gguf</code> model files in your models directory (default: <code>~/.squig-models/</code>). Or use the <strong>🤗 HuggingFace</strong> tab to search and download models directly.</p>
        <p>Supported model formats: Any GGUF model compatible with llama.cpp — Llama, Qwen, DeepSeek, Mistral, Phi, Gemma, Command-R, and more.</p>
      </div>

      <div class="help-card">
        <h3>2. Load a Model</h3>
        <p>Go to the <strong>Models</strong> tab and click <strong>Load Model</strong> on the model you want to use. The server will start a llama-server process in the background.</p>
        <p>You can also set <code>default_model</code> in <code>config.toml</code> to auto-load a model on startup.</p>
      </div>

      <div class="help-card">
        <h3>3. Chat or Connect</h3>
        <p>Use the built-in <strong>Chat</strong> tab for quick conversations, or connect any OpenAI-compatible app to <code>http://&lt;your-ip&gt;:9090</code>.</p>
      </div>

      <div class="help-card">
        <h3>4. Tune Performance</h3>
        <p>Open the <strong>🔧 Developer</strong> tab to adjust GPU layers, context size, flash attention, KV cache quantization, and 15+ other settings. Use <strong>AI Optimize</strong> for automatic tuning based on your hardware.</p>
      </div>

      <div class="help-card highlight">
        <h3>Quick Architecture Overview</h3>
        <div class="arch-diagram">
          <div class="arch-layer">
            <span class="arch-box app">Desktop App (Tauri 2)</span>
          </div>
          <div class="arch-arrow">↕</div>
          <div class="arch-layer">
            <span class="arch-box server">Rust/Axum Server (:9090)</span>
          </div>
          <div class="arch-arrow">↕</div>
          <div class="arch-layer">
            <span class="arch-box engine">llama.cpp (CUDA / Vulkan / CPU)</span>
          </div>
        </div>
        <p class="arch-note">The Rust server manages the llama.cpp inference engine, proxies OpenAI-compatible requests, and serves the UI.</p>
      </div>

    {:else if activeSection === 'models'}
      <h2>Models</h2>

      <div class="help-card">
        <h3>Model Directories</h3>
        <p>Models are scanned from directories listed in <code>config.toml</code>:</p>
        <pre><code>[models]
directories = [
    "/home/you/.squig-models",
    "/mnt/models",
]</code></pre>
        <p>Subdirectories are scanned recursively. Models appear automatically in the <strong>Models</strong> tab.</p>
      </div>

      <div class="help-card">
        <h3>Downloading from HuggingFace</h3>
        <p>The <strong>🤗 HuggingFace</strong> tab lets you search for GGUF models and download them directly. It shows:</p>
        <ul>
          <li><strong>Hardware fit ratings</strong> — whether a model will fit in your VRAM/RAM</li>
          <li><strong>File sizes</strong> — fetched from repo metadata</li>
          <li><strong>Download progress</strong> — with cancel support</li>
          <li><strong>Auto-load</strong> — optionally loads the model as soon as it finishes downloading</li>
        </ul>
      </div>

      <div class="help-card">
        <h3>Managing Models</h3>
        <ul>
          <li><strong>Load/Unload</strong> — from the Models tab; each model runs its own llama-server process</li>
          <li><strong>Delete</strong> — removes the GGUF file from disk (with confirmation)</li>
          <li><strong>Max loaded</strong> — <code>max_loaded_models</code> in config controls how many can run simultaneously</li>
        </ul>
      </div>

      <div class="help-card">
        <h3>Recommended Models for Your Hardware</h3>
        <table class="help-table">
          <thead>
            <tr><th>VRAM</th><th>Model Size</th><th>Examples</th></tr>
          </thead>
          <tbody>
            <tr><td>4 GB</td><td>3B Q4</td><td>Qwen2.5-3B, Phi-3-mini</td></tr>
            <tr><td>8 GB</td><td>7-8B Q4</td><td>Llama-3.1-8B, Qwen2.5-7B</td></tr>
            <tr><td>16 GB</td><td>14B Q5 / 32B Q4</td><td>Qwen2.5-14B, DeepSeek-R1-32B</td></tr>
            <tr><td>24 GB</td><td>32B Q5 / 70B Q4</td><td>Qwen2.5-32B, Llama-3.1-70B</td></tr>
            <tr><td>48 GB+</td><td>70B+ Q5+</td><td>Llama-3.1-70B Q5, larger MoE models</td></tr>
          </tbody>
        </table>
      </div>

    {:else if activeSection === 'settings'}
      <h2>Settings Reference</h2>
      <p class="intro">All settings are in <code>config.toml</code> and can be changed live from the <strong>🔧 Developer</strong> tab.</p>

      <div class="help-card">
        <h3>Server Settings</h3>
        <table class="help-table">
          <thead><tr><th>Setting</th><th>Default</th><th>Description</th></tr></thead>
          <tbody>
            <tr><td><code>host</code></td><td><code>0.0.0.0</code></td><td>Bind address. <code>0.0.0.0</code> = accessible from network</td></tr>
            <tr><td><code>port</code></td><td><code>9090</code></td><td>HTTP port for API and UI</td></tr>
            <tr><td><code>max_concurrent_requests</code></td><td><code>16</code></td><td>Max parallel API requests</td></tr>
            <tr><td><code>api_key</code></td><td><em>(empty)</em></td><td>Optional API key for authentication</td></tr>
          </tbody>
        </table>
      </div>

      <div class="help-card">
        <h3>Inference Settings</h3>
        <table class="help-table">
          <thead><tr><th>Setting</th><th>Default</th><th>Description</th></tr></thead>
          <tbody>
            <tr><td><code>gpu_layers</code></td><td><code>-1</code></td><td>Layers to offload to GPU. <code>-1</code> = all, <code>0</code> = CPU only</td></tr>
            <tr><td><code>context_size</code></td><td><code>32768</code></td><td>Context window in tokens</td></tr>
            <tr><td><code>parallel_slots</code></td><td><code>4</code></td><td>Continuous batching slots for concurrent requests</td></tr>
            <tr><td><code>flash_attention</code></td><td><code>true</code></td><td>Reduces KV cache memory ~50%</td></tr>
            <tr><td><code>gpu_backend</code></td><td><code>"auto"</code></td><td>One of: <code>auto</code>, <code>cuda</code>, <code>vulkan</code>, <code>rocm</code>, <code>cpu</code></td></tr>
            <tr><td><code>kv_cache_type_k</code></td><td><code>"q8_0"</code></td><td>KV cache quantization for Keys</td></tr>
            <tr><td><code>kv_cache_type_v</code></td><td><code>"q8_0"</code></td><td>KV cache quantization for Values (can differ from K)</td></tr>
            <tr><td><code>threads</code></td><td><code>-1</code></td><td>CPU threads. <code>-1</code> = auto-detect</td></tr>
            <tr><td><code>threads_batch</code></td><td><code>-1</code></td><td>Batch processing threads. <code>-1</code> = same as threads</td></tr>
            <tr><td><code>batch_size</code></td><td><code>2048</code></td><td>Logical batch size for prompt processing</td></tr>
            <tr><td><code>ubatch_size</code></td><td><code>512</code></td><td>Physical batch size</td></tr>
            <tr><td><code>mlock</code></td><td><code>false</code></td><td>Lock model memory (prevents swapping)</td></tr>
            <tr><td><code>no_mmap</code></td><td><code>false</code></td><td>Disable memory-mapped loading</td></tr>
            <tr><td><code>n_predict</code></td><td><code>-1</code></td><td>Max tokens per response. <code>-1</code> = unlimited</td></tr>
          </tbody>
        </table>
      </div>

      <div class="help-card">
        <h3>RoPE Settings</h3>
        <table class="help-table">
          <thead><tr><th>Setting</th><th>Default</th><th>Description</th></tr></thead>
          <tbody>
            <tr><td><code>rope_scaling</code></td><td><em>(empty)</em></td><td>RoPE scaling: <code>none</code>, <code>linear</code>, <code>yarn</code></td></tr>
            <tr><td><code>rope_freq_base</code></td><td><code>0</code></td><td>RoPE base frequency (0 = model default)</td></tr>
            <tr><td><code>rope_freq_scale</code></td><td><code>0</code></td><td>RoPE frequency scale (0 = model default)</td></tr>
          </tbody>
        </table>
      </div>

      <div class="help-card">
        <h3>Multi-GPU Settings</h3>
        <table class="help-table">
          <thead><tr><th>Setting</th><th>Default</th><th>Description</th></tr></thead>
          <tbody>
            <tr><td><code>split_mode</code></td><td><code>"layer"</code></td><td>How to split model across GPUs: <code>none</code>, <code>layer</code>, <code>row</code></td></tr>
            <tr><td><code>main_gpu</code></td><td><code>0</code></td><td>Primary GPU index</td></tr>
            <tr><td><code>tensor_split</code></td><td><em>(empty)</em></td><td>GPU split ratios, e.g. <code>"3,1"</code></td></tr>
          </tbody>
        </table>
      </div>

      <div class="help-card">
        <h3>Speculative Decoding</h3>
        <table class="help-table">
          <thead><tr><th>Setting</th><th>Default</th><th>Description</th></tr></thead>
          <tbody>
            <tr><td><code>speculative.enabled</code></td><td><code>false</code></td><td>Enable speculative decoding (2-3x speedup)</td></tr>
            <tr><td><code>speculative.draft_model</code></td><td><em>(empty)</em></td><td>Path to small draft model GGUF</td></tr>
            <tr><td><code>speculative.draft_max</code></td><td><code>16</code></td><td>Max tokens to draft per step</td></tr>
            <tr><td><code>speculative.draft_min</code></td><td><code>4</code></td><td>Min tokens to draft per step</td></tr>
          </tbody>
        </table>
      </div>

      <div class="help-card">
        <h3>KV Cache Type Options</h3>
        <p>Both <code>kv_cache_type_k</code> and <code>kv_cache_type_v</code> accept these values:</p>
        <table class="help-table compact">
          <thead><tr><th>Type</th><th>Bits/value</th><th>Quality</th><th>VRAM Savings</th></tr></thead>
          <tbody>
            <tr><td><code>f32</code></td><td>32</td><td>Maximum precision</td><td>None (baseline)</td></tr>
            <tr><td><code>f16</code></td><td>16</td><td>Near-perfect</td><td>50%</td></tr>
            <tr><td><code>q8_0</code></td><td>8</td><td>Excellent (recommended)</td><td>75%</td></tr>
            <tr><td><code>q5_1</code></td><td>~5.5</td><td>Very good</td><td>~83%</td></tr>
            <tr><td><code>q5_0</code></td><td>~5</td><td>Good</td><td>~84%</td></tr>
            <tr><td><code>q4_1</code></td><td>~4.5</td><td>Good</td><td>~86%</td></tr>
            <tr><td><code>q4_0</code></td><td>4</td><td>Acceptable</td><td>~87%</td></tr>
            <tr><td><code>iq4_nl</code></td><td>~4</td><td>Acceptable</td><td>~87%</td></tr>
          </tbody>
        </table>
        <p class="tip">💡 Setting V to a lower quant than K (e.g. K=q8_0, V=q4_0) often saves significant VRAM with little quality loss.</p>
      </div>

    {:else if activeSection === 'api'}
      <h2>API & Server Setup</h2>
      <p class="intro">Squig Model Server exposes an <strong>OpenAI-compatible API</strong>, so any app that works with OpenAI can connect directly.</p>

      <div class="help-card">
        <h3>Network Access</h3>
        <p>The server binds to <code>0.0.0.0:9090</code> by default — accessible from any device on your local network.</p>
        <pre><code># Find your machine's IP:
ip addr show  # Linux
ipconfig      # Windows

# Access from another device:
http://192.168.x.x:9090</code></pre>
      </div>

      <div class="help-card">
        <h3>API Endpoints</h3>
        <table class="help-table">
          <thead><tr><th>Endpoint</th><th>Method</th><th>Description</th></tr></thead>
          <tbody>
            <tr><td><code>/v1/chat/completions</code></td><td>POST</td><td>Chat completions (streaming & non-streaming)</td></tr>
            <tr><td><code>/v1/completions</code></td><td>POST</td><td>Text completions</td></tr>
            <tr><td><code>/v1/models</code></td><td>GET</td><td>List available models</td></tr>
            <tr><td><code>/v1/models/:id</code></td><td>GET</td><td>Get model info</td></tr>
          </tbody>
        </table>
      </div>

      <div class="help-card">
        <h3>Connecting Apps</h3>
        <p>In any app that supports OpenAI-compatible APIs, set:</p>
        <table class="help-table">
          <thead><tr><th>Field</th><th>Value</th></tr></thead>
          <tbody>
            <tr><td>API Base URL</td><td><code>http://&lt;your-ip&gt;:9090/v1</code></td></tr>
            <tr><td>API Key</td><td><code>not-needed</code> (or your key from config.toml)</td></tr>
            <tr><td>Model</td><td>Any loaded model name, or leave blank</td></tr>
          </tbody>
        </table>
      </div>

      <div class="help-card">
        <h3>Python (OpenAI SDK)</h3>
        <pre><code>from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:9090/v1",
    api_key="not-needed",
)

response = client.chat.completions.create(
    model="qwen2.5-3b-instruct-q4_k_m",
    messages=[
        {"{"}
            "role": "user",
            "content": "Hello!"
        {"}"}
    ],
    stream=True,
)

for chunk in response:
    print(chunk.choices[0].delta.content or "", end="")</code></pre>
      </div>

      <div class="help-card">
        <h3>curl</h3>
        <pre><code>curl http://localhost:9090/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"{"}
    "model": "qwen2.5-3b-instruct-q4_k_m",
    "messages": [
      {"{"}"role": "user", "content": "Hello!"{"}"}
    ]
  {"}"}'</code></pre>
      </div>

      <div class="help-card">
        <h3>Popular Apps</h3>
        <table class="help-table">
          <thead><tr><th>App</th><th>Configuration</th></tr></thead>
          <tbody>
            <tr><td><strong>Open WebUI</strong></td><td>Settings → Connections → OpenAI API: <code>http://your-ip:9090/v1</code></td></tr>
            <tr><td><strong>SillyTavern</strong></td><td>API → Chat Completion → Custom (OpenAI-compatible) → <code>http://your-ip:9090</code></td></tr>
            <tr><td><strong>Continue.dev</strong></td><td>Add OpenAI-compatible provider with base URL <code>http://localhost:9090/v1</code></td></tr>
            <tr><td><strong>LangChain</strong></td><td><code>ChatOpenAI(base_url="http://localhost:9090/v1", api_key="x")</code></td></tr>
            <tr><td><strong>Aider</strong></td><td><code>--openai-api-base http://localhost:9090/v1</code></td></tr>
          </tbody>
        </table>
      </div>

    {:else if activeSection === 'tools'}
      <h2>Tool Calling / Function Calling</h2>
      <p class="intro">Squig Model Server supports OpenAI-compatible tool calling for AI agents, coding assistants, and function-calling workflows.</p>

      <div class="help-card">
        <h3>How It Works</h3>
        <p>When you send <code>tools</code> in a chat completion request, the model can choose to call one or more functions instead of (or alongside) generating text. The server uses Jinja2 templates to format tool definitions for the model.</p>
        <ul>
          <li>Supports <code>tools</code>, <code>tool_choice</code>, and <code>parallel_tool_calls</code></li>
          <li>Works with streaming and non-streaming responses</li>
          <li>Compatible with LangChain, CrewAI, AutoGen, and other agent frameworks</li>
        </ul>
      </div>

      <div class="help-card">
        <h3>Example: Tool Calling with Python</h3>
        <pre><code>from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:9090/v1",
    api_key="not-needed",
)

tools = [
    {"{"}
        "type": "function",
        "function": {"{"}
            "name": "get_weather",
            "description": "Get current weather for a location",
            "parameters": {"{"}
                "type": "object",
                "properties": {"{"}
                    "location": {"{"}
                        "type": "string",
                        "description": "City name"
                    {"}"}
                {"}"},
                "required": ["location"]
            {"}"}
        {"}"}
    {"}"}
]

response = client.chat.completions.create(
    model="qwen2.5-3b-instruct-q4_k_m",
    messages=[{"{"}"role": "user", "content": "What's the weather in Tokyo?"{"}"}],
    tools=tools,
    tool_choice="auto",
)

# The model may return tool_calls in the response
message = response.choices[0].message
if message.tool_calls:
    for call in message.tool_calls:
        print(f"Call: {"{"}call.function.name{"}"}")
        print(f"Args: {"{"}call.function.arguments{"}"}")</code></pre>
      </div>

      <div class="help-card">
        <h3>Best Models for Tool Calling</h3>
        <ul>
          <li><strong>Qwen2.5-Coder</strong> — excellent at structured output and function calls</li>
          <li><strong>Qwen3-Coder</strong> — latest generation, top tier tool use</li>
          <li><strong>Llama 3.1+</strong> — good tool calling support with instruct variants</li>
          <li><strong>DeepSeek-V2/R1</strong> — strong at following tool schemas</li>
          <li><strong>Mistral/Mixtral</strong> — native function calling support</li>
        </ul>
        <p class="tip">💡 Models must support chat templates with tool definitions. Most modern instruct models do.</p>
      </div>

    {:else if activeSection === 'tips'}
      <h2>Tips & Tricks</h2>

      <div class="help-card">
        <h3>🎯 Get More VRAM for Bigger Models</h3>
        <ul>
          <li>Use <strong>Flash Attention</strong> — saves ~50% KV cache memory</li>
          <li>Set KV cache to <code>q8_0</code> or lower — saves another 50%+ on cache</li>
          <li>Set K and V separately — V can go lower than K with minimal quality loss</li>
          <li>Reduce <code>context_size</code> if you don't need long conversations</li>
          <li>Reduce <code>parallel_slots</code> to 1 if you're the only user</li>
        </ul>
      </div>

      <div class="help-card">
        <h3>⚡ Maximize Speed</h3>
        <ul>
          <li>Use <strong>CUDA</strong> backend for NVIDIA GPUs (fastest)</li>
          <li>Use <strong>Vulkan</strong> for AMD/Intel GPUs</li>
          <li>Set <code>gpu_layers = -1</code> to offload everything to GPU</li>
          <li>Enable <strong>speculative decoding</strong> with a small draft model for 2-3x speedup</li>
          <li>Set <code>threads</code> to your physical core count (not hyperthreads)</li>
          <li>Use <code>mlock = true</code> if you have enough RAM (prevents swapping)</li>
        </ul>
      </div>

      <div class="help-card">
        <h3>🌐 Running as a Network Server</h3>
        <ul>
          <li>Default bind is <code>0.0.0.0:9090</code> — already network-accessible</li>
          <li>Open port 9090 in your firewall if needed</li>
          <li>Set an <code>api_key</code> in config.toml for security on shared networks</li>
          <li>Connect from any device: <code>http://&lt;server-ip&gt;:9090</code></li>
          <li>Multiple clients can connect simultaneously (up to <code>parallel_slots</code> concurrent inferences)</li>
        </ul>
      </div>

      <div class="help-card">
        <h3>🐛 Troubleshooting</h3>
        <table class="help-table">
          <thead><tr><th>Problem</th><th>Solution</th></tr></thead>
          <tbody>
            <tr><td>Model won't load</td><td>Check GPU memory — try a smaller quantization (Q4 vs Q5)</td></tr>
            <tr><td>Slow generation</td><td>Ensure model fits entirely in VRAM (<code>gpu_layers = -1</code>)</td></tr>
            <tr><td>Out of memory</td><td>Reduce context_size, parallel_slots, or use lower KV cache quant</td></tr>
            <tr><td>Blank screen / WebKit error</td><td>On NVIDIA+Wayland, set <code>WEBKIT_DISABLE_DMABUF_RENDERER=1</code></td></tr>
            <tr><td>Can't connect from network</td><td>Check firewall allows port 9090; verify host is <code>0.0.0.0</code></td></tr>
            <tr><td>HuggingFace search fails</td><td>Check internet connection; the server needs to reach huggingface.co</td></tr>
          </tbody>
        </table>
      </div>

      <div class="help-card">
        <h3>📁 File Locations</h3>
        <table class="help-table">
          <thead><tr><th>File</th><th>Path</th><th>Purpose</th></tr></thead>
          <tbody>
            <tr><td>Config</td><td><code>config.toml</code></td><td>All server/inference settings</td></tr>
            <tr><td>Models</td><td><code>~/.squig-models/</code></td><td>Default model storage directory</td></tr>
            <tr><td>llama-server</td><td>Configured in <code>[inference.backend_paths]</code></td><td>llama.cpp inference binary</td></tr>
          </tbody>
        </table>
      </div>

    {:else if activeSection === 'about'}
      <h2>About Squig AI</h2>
      <p class="intro">Squig Model Server is built by <strong>Squig AI</strong> — making powerful local AI accessible to everyone.</p>

      <div class="help-card about-hero">
        <div class="about-logo">✦ Squig AI</div>
        <p class="about-tagline">Run AI on your hardware. No cloud. No subscriptions. No limits.</p>
      </div>

      <div class="help-card">
        <h3>What is Squig Model Server?</h3>
        <p>Squig Model Server is a <strong>free, open-source</strong> desktop application that lets you run large language models entirely on your own machine. It wraps llama.cpp in a polished UI with hardware auto-detection, smart parameter tuning, and a full OpenAI-compatible API — so you can use local models with any app that supports OpenAI.</p>
        <ul>
          <li><strong>Zero cloud dependency</strong> — your data never leaves your machine</li>
          <li><strong>Multi-backend GPU acceleration</strong> — CUDA, Vulkan, ROCm, or CPU</li>
          <li><strong>Smart defaults</strong> — automatically tunes context size, KV cache, and GPU layers to fit your hardware</li>
          <li><strong>HuggingFace integration</strong> — search, download, and load models in one click</li>
          <li><strong>OpenAI-compatible API</strong> — works with Open WebUI, SillyTavern, Continue.dev, LangChain, and more</li>
          <li><strong>Tool/function calling</strong> — full support for AI agents and coding assistants</li>
          <li><strong>AI-powered auto-optimizer</strong> — tunes settings based on your hardware and performance data</li>
        </ul>
      </div>

      <div class="help-card">
        <h3>The Squig AI Ecosystem</h3>
        <p>Squig Model Server is part of a growing suite of AI-powered tools from Squig AI:</p>
        <table class="help-table">
          <thead><tr><th>Product</th><th>Description</th></tr></thead>
          <tbody>
            <tr><td><strong>Squig Model Server</strong></td><td>Local LLM inference server with desktop app</td></tr>
            <tr><td><strong>Squig Assistant</strong></td><td>AI-powered desktop assistant</td></tr>
            <tr><td><strong>Squig Power Code</strong></td><td>AI coding environment and developer tools</td></tr>
            <tr><td><strong>Squig Trainer</strong></td><td>Model fine-tuning and training toolkit</td></tr>
            <tr><td><strong>Squig Suite</strong></td><td>AI-enhanced office productivity suite</td></tr>
            <tr><td><strong>Squig Budget</strong></td><td>AI-powered personal finance and budgeting</td></tr>
            <tr><td><strong>Squig Freelance</strong></td><td>AI tools for freelancers — invoicing, clients, and projects</td></tr>
          </tbody>
        </table>
      </div>

      <div class="help-card">
        <h3>Links</h3>
        <div class="about-links">
          <a href="https://squig-ai.com" target="_blank" rel="noopener noreferrer">🌐 squig-ai.com</a>
          <a href="https://github.com/hayesjl77/Squig-Model-Server" target="_blank" rel="noopener noreferrer">📂 GitHub Repository</a>
        </div>
      </div>

      <div class="help-card">
        <h3>License</h3>
        <p>Squig Model Server is released under the <strong>MIT License</strong>. Free to use, modify, and distribute.</p>
        <p class="about-copy">© 2026 Squig AI. All rights reserved.</p>
      </div>

    {/if}
  </div>
</div>

<style>
  .help-panel {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 1.5rem;
    max-width: 1100px;
    margin: 0 auto;
    min-height: 600px;
  }

  .help-sidebar {
    position: sticky;
    top: 1rem;
    align-self: start;
  }

  .help-sidebar h2 {
    font-size: 1.1rem;
    color: #e0e0e0;
    margin-bottom: 1rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid #1e1e30;
  }

  .help-sidebar nav {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .help-sidebar button {
    background: none;
    border: none;
    padding: 0.6rem 0.8rem;
    border-radius: 6px;
    color: #888;
    cursor: pointer;
    text-align: left;
    font-size: 0.88rem;
    transition: all 0.15s;
  }
  .help-sidebar button:hover { color: #ccc; background: #12121f; }
  .help-sidebar button.active { color: #a78bfa; background: #1a1a2e; font-weight: 600; }

  .help-content {
    padding-bottom: 2rem;
  }

  .help-content h2 {
    font-size: 1.5rem;
    color: #e8e8f0;
    margin-bottom: 0.5rem;
  }

  .intro {
    color: #888;
    font-size: 0.95rem;
    margin-bottom: 1.5rem;
    line-height: 1.6;
  }

  .help-card {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 10px;
    padding: 1.25rem;
    margin-bottom: 1rem;
  }
  .help-card.highlight {
    border-color: #2a2a50;
    background: #0c0c18;
  }

  .help-card h3 {
    font-size: 1rem;
    color: #d0d0e0;
    margin-bottom: 0.75rem;
  }

  .help-card p {
    color: #999;
    font-size: 0.9rem;
    line-height: 1.6;
    margin-bottom: 0.5rem;
  }
  .help-card p:last-child { margin-bottom: 0; }

  .help-card ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .help-card ul li {
    color: #999;
    font-size: 0.88rem;
    line-height: 1.7;
    padding-left: 1.2rem;
    position: relative;
  }

  .help-card ul li::before {
    content: '•';
    color: #a78bfa;
    position: absolute;
    left: 0;
  }

  .help-card code {
    background: #161625;
    color: #a78bfa;
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    font-size: 0.84rem;
    font-family: 'Fira Code', 'Cascadia Code', 'JetBrains Mono', monospace;
  }

  .help-card pre {
    background: #0a0a16;
    border: 1px solid #1a1a2e;
    border-radius: 6px;
    padding: 1rem;
    overflow-x: auto;
    margin: 0.5rem 0;
  }

  .help-card pre code {
    background: none;
    padding: 0;
    color: #b0b0c8;
    font-size: 0.82rem;
    line-height: 1.6;
  }

  .tip {
    color: #a78bfa !important;
    font-style: italic;
    margin-top: 0.5rem;
  }

  /* Tables */
  .help-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
    margin: 0.5rem 0;
  }

  .help-table th {
    text-align: left;
    padding: 0.5rem 0.75rem;
    color: #aaa;
    border-bottom: 1px solid #1e1e30;
    font-weight: 600;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .help-table td {
    padding: 0.45rem 0.75rem;
    color: #999;
    border-bottom: 1px solid #12121f;
  }

  .help-table td code {
    font-size: 0.8rem;
  }

  .help-table.compact td { padding: 0.3rem 0.6rem; }

  .help-table tbody tr:hover td {
    background: #0a0a16;
  }

  /* Architecture diagram */
  .arch-diagram {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    margin: 1rem 0;
  }

  .arch-layer {
    display: flex;
    justify-content: center;
  }

  .arch-box {
    padding: 0.6rem 1.5rem;
    border-radius: 8px;
    font-size: 0.88rem;
    font-weight: 600;
    text-align: center;
    min-width: 280px;
  }

  .arch-box.app { background: #1a1a3a; color: #a78bfa; border: 1px solid #2a2a50; }
  .arch-box.server { background: #1a2e28; color: #6ee7b7; border: 1px solid #2a4a3a; }
  .arch-box.engine { background: #2a1a0e; color: #fbbf24; border: 1px solid #4a3018; }

  .arch-arrow { color: #444; font-size: 1.2rem; }

  .arch-note {
    text-align: center;
    font-size: 0.82rem !important;
    color: #666 !important;
    margin-top: 0.5rem;
  }

  /* About section */
  .about-hero {
    text-align: center;
    padding: 2rem 1.5rem !important;
    background: linear-gradient(135deg, #0e0e1a 0%, #1a1a3a 100%) !important;
    border: 1px solid #2a2a50 !important;
  }

  .about-logo {
    font-size: 2rem;
    font-weight: 700;
    color: #a78bfa;
    margin-bottom: 0.75rem;
    letter-spacing: 0.5px;
  }

  .about-tagline {
    font-size: 1.1rem !important;
    color: #bbb !important;
    font-weight: 400;
  }

  .about-links {
    display: flex;
    gap: 1.5rem;
    flex-wrap: wrap;
  }

  .about-links a {
    color: #a78bfa;
    text-decoration: none;
    font-weight: 500;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    background: #12121f;
    border: 1px solid #1e1e30;
    transition: all 0.15s;
  }
  .about-links a:hover {
    background: #1a1a2e;
    border-color: #a78bfa;
    color: #c4b5fd;
  }

  .about-copy {
    color: #555 !important;
    font-size: 0.82rem !important;
    margin-top: 0.5rem;
  }

  /* Responsive */
  @media (max-width: 768px) {
    .help-panel {
      grid-template-columns: 1fr;
    }
    .help-sidebar {
      position: static;
    }
    .help-sidebar nav {
      flex-direction: row;
      flex-wrap: wrap;
    }
  }
</style>
