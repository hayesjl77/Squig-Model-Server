<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';
  import AutoOptimize from './AutoOptimize.svelte';

  // --- Tuning Settings State ---
  let settings = $state(null);
  let settingsLoading = $state(true);
  let settingsDirty = $state(false);
  let settingsApplying = $state(false);
  let settingsError = $state('');
  let settingsSuccess = $state('');
  let loadedModelsList = $state([]);
  let reloadAfterApply = $state(true);
  let saveToDisk = $state(false);

  // Editable copies of settings
  let gpuLayers = $state(-1);
  let contextSize = $state(4096);
  let parallelSlots = $state(1);
  let flashAttention = $state(true);
  let continuousBatching = $state(true);
  let kvCacheTypeK = $state('q8_0');
  let kvCacheTypeV = $state('q8_0');
  let gpuBackend = $state('vulkan');
  let availableBackends = $state([]);
  // New settings
  let threads = $state(-1);
  let threadsBatch = $state(-1);
  let batchSize = $state(2048);
  let ubatchSize = $state(512);
  let mlock = $state(false);
  let noMmap = $state(false);
  let nPredict = $state(-1);
  let ropeScaling = $state('');
  let ropeFreqBase = $state(0);
  let ropeFreqScale = $state(0);
  let splitMode = $state('layer');
  let mainGpu = $state(0);
  let tensorSplit = $state('');
  let cachePrompt = $state(true);
  let warmup = $state(true);
  let smartDefaults = $state(true);

  // Hardware detection
  let cpuCores = $state(0);
  let cpuThreads = $state(0);

  async function loadSettings() {
    try {
      const [s, lm, hw] = await Promise.all([
        api.devSettings(),
        api.loadedModels(),
        api.hardware(),
      ]);
      settings = s;
      loadedModelsList = lm.models || [];

      // Populate editable fields from server state
      gpuLayers = s.gpu_layers ?? -1;
      contextSize = s.context_size ?? 4096;
      parallelSlots = s.parallel_slots ?? 1;
      flashAttention = s.flash_attention ?? true;
      continuousBatching = s.continuous_batching ?? true;
      kvCacheTypeK = s.kv_cache_type_k ?? 'q8_0';
      kvCacheTypeV = s.kv_cache_type_v ?? 'q8_0';
      gpuBackend = s.gpu_backend ?? 'vulkan';
      availableBackends = s.available_backends ?? [];
      threads = s.threads ?? -1;
      threadsBatch = s.threads_batch ?? -1;
      batchSize = s.batch_size ?? 2048;
      ubatchSize = s.ubatch_size ?? 512;
      mlock = s.mlock ?? false;
      noMmap = s.no_mmap ?? false;
      nPredict = s.n_predict ?? -1;
      ropeScaling = s.rope_scaling ?? '';
      ropeFreqBase = s.rope_freq_base ?? 0;
      ropeFreqScale = s.rope_freq_scale ?? 0;
      splitMode = s.split_mode ?? 'layer';
      mainGpu = s.main_gpu ?? 0;
      tensorSplit = s.tensor_split ?? '';
      cachePrompt = s.cache_prompt ?? true;
      warmup = s.warmup ?? true;
      smartDefaults = s.smart_defaults ?? true;

      // Hardware info
      cpuCores = hw.cpu_cores || 0;
      cpuThreads = hw.cpu_threads || 0;

      settingsDirty = false;
      settingsError = '';
    } catch (e) {
      settingsError = `Failed to load settings: ${e.message}`;
    } finally {
      settingsLoading = false;
    }
  }

  function markDirty() {
    settingsDirty = true;
    settingsSuccess = '';
  }

  function buildChanges() {
    if (!settings) return [];
    const changes = [];
    if (gpuLayers !== settings.gpu_layers) changes.push({ setting: 'gpu_layers', value: gpuLayers });
    if (contextSize !== settings.context_size) changes.push({ setting: 'context_size', value: contextSize });
    if (parallelSlots !== settings.parallel_slots) changes.push({ setting: 'parallel_slots', value: parallelSlots });
    if (flashAttention !== settings.flash_attention) changes.push({ setting: 'flash_attention', value: flashAttention });
    if (continuousBatching !== settings.continuous_batching) changes.push({ setting: 'continuous_batching', value: continuousBatching });
    if (kvCacheTypeK !== settings.kv_cache_type_k) changes.push({ setting: 'kv_cache_type_k', value: kvCacheTypeK });
    if (kvCacheTypeV !== settings.kv_cache_type_v) changes.push({ setting: 'kv_cache_type_v', value: kvCacheTypeV });
    if (gpuBackend !== settings.gpu_backend) changes.push({ setting: 'gpu_backend', value: gpuBackend });
    if (threads !== settings.threads) changes.push({ setting: 'threads', value: threads });
    if (threadsBatch !== settings.threads_batch) changes.push({ setting: 'threads_batch', value: threadsBatch });
    if (batchSize !== settings.batch_size) changes.push({ setting: 'batch_size', value: batchSize });
    if (ubatchSize !== settings.ubatch_size) changes.push({ setting: 'ubatch_size', value: ubatchSize });
    if (mlock !== settings.mlock) changes.push({ setting: 'mlock', value: mlock });
    if (noMmap !== settings.no_mmap) changes.push({ setting: 'no_mmap', value: noMmap });
    if (nPredict !== settings.n_predict) changes.push({ setting: 'n_predict', value: nPredict });
    if (ropeScaling !== settings.rope_scaling) changes.push({ setting: 'rope_scaling', value: ropeScaling });
    if (ropeFreqBase !== settings.rope_freq_base) changes.push({ setting: 'rope_freq_base', value: ropeFreqBase });
    if (ropeFreqScale !== settings.rope_freq_scale) changes.push({ setting: 'rope_freq_scale', value: ropeFreqScale });
    if (splitMode !== settings.split_mode) changes.push({ setting: 'split_mode', value: splitMode });
    if (mainGpu !== settings.main_gpu) changes.push({ setting: 'main_gpu', value: mainGpu });
    if (tensorSplit !== settings.tensor_split) changes.push({ setting: 'tensor_split', value: tensorSplit });
    if (cachePrompt !== settings.cache_prompt) changes.push({ setting: 'cache_prompt', value: cachePrompt });
    if (warmup !== settings.warmup) changes.push({ setting: 'warmup', value: warmup });
    if (smartDefaults !== settings.smart_defaults) changes.push({ setting: 'smart_defaults', value: smartDefaults });
    return changes;
  }

  async function applySettings() {
    const changes = buildChanges();
    if (changes.length === 0) return;

    settingsApplying = true;
    settingsError = '';
    settingsSuccess = '';

    const modelName = loadedModelsList[0]?.name;

    try {
      const res = await api.devApplySettings(
        changes,
        reloadAfterApply && modelName ? modelName : undefined,
        saveToDisk,
      );

      if (res.errors?.length > 0) {
        settingsError = res.errors.join('; ');
      }
      if (res.applied?.length > 0) {
        settingsSuccess = `Applied: ${res.applied.join(', ')}`;
      }
      if (res.reload_status === 'reloaded') {
        settingsSuccess += ' | Model reloaded.';
      }

      // Refresh from server
      await loadSettings();
    } catch (e) {
      settingsError = e.message;
    } finally {
      settingsApplying = false;
    }
  }

  function resetSettings() {
    if (!settings) return;
    gpuLayers = settings.gpu_layers;
    contextSize = settings.context_size;
    parallelSlots = settings.parallel_slots;
    flashAttention = settings.flash_attention;
    continuousBatching = settings.continuous_batching;
    kvCacheTypeK = settings.kv_cache_type_k;
    kvCacheTypeV = settings.kv_cache_type_v;
    gpuBackend = settings.gpu_backend;
    threads = settings.threads;
    threadsBatch = settings.threads_batch;
    batchSize = settings.batch_size;
    ubatchSize = settings.ubatch_size;
    mlock = settings.mlock;
    noMmap = settings.no_mmap;
    nPredict = settings.n_predict;
    ropeScaling = settings.rope_scaling;
    ropeFreqBase = settings.rope_freq_base;
    ropeFreqScale = settings.rope_freq_scale;
    splitMode = settings.split_mode;
    mainGpu = settings.main_gpu;
    tensorSplit = settings.tensor_split;
    cachePrompt = settings.cache_prompt;
    warmup = settings.warmup;
    smartDefaults = settings.smart_defaults;
    settingsDirty = false;
    settingsSuccess = '';
    settingsError = '';
  }

  // Context size presets
  const ctxPresets = [2048, 4096, 8192, 16384, 32768, 65536, 131072];

  // --- API Log State ---
  let logs = $state([]);
  let logsLoading = $state(true);
  let autoRefresh = $state(true);
  let modelFilter = $state('');
  let expandedEntry = $state(null);
  let showRequestBody = $state(true);
  let showResponseBody = $state(true);
  let activeSection = $state('tuning'); // 'tuning' | 'logs'

  async function refreshLogs() {
    try {
      const res = await api.devLogs(200, modelFilter || undefined);
      logs = res.entries || [];
    } catch (_) {}
    logsLoading = false;
  }

  async function clearLogs() {
    await api.devLogsClear();
    logs = [];
  }

  function toggleEntry(id) {
    expandedEntry = expandedEntry === id ? null : id;
  }

  function statusColor(code) {
    if (code >= 200 && code < 300) return '#6ee7b7';
    if (code >= 400 && code < 500) return '#fbbf24';
    return '#f87171';
  }

  function tpsColor(tps) {
    if (tps >= 30) return '#6ee7b7';
    if (tps >= 15) return '#34d399';
    if (tps >= 5) return '#fbbf24';
    if (tps > 0) return '#f87171';
    return '#555';
  }

  function formatTime(timestamp) {
    if (!timestamp) return '';
    try {
      const d = new Date(timestamp);
      return d.toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
    } catch { return timestamp; }
  }

  function formatJson(str) {
    try {
      return JSON.stringify(JSON.parse(str), null, 2);
    } catch { return str; }
  }

  onMount(() => {
    loadSettings();
    refreshLogs();
  });

  $effect(() => {
    if (autoRefresh) {
      const interval = setInterval(refreshLogs, 2000);
      return () => clearInterval(interval);
    }
  });
</script>

<div class="dev-panel">
  <!-- Section Tabs -->
  <div class="section-tabs">
    <button class:active={activeSection === 'tuning'} onclick={() => activeSection = 'tuning'}>⚙️ Tuning Settings</button>
    <button class:active={activeSection === 'logs'} onclick={() => activeSection = 'logs'}>🔧 API Request Log <span class="tab-badge">{logs.length}</span></button>
    <button class:active={activeSection === 'optimize'} onclick={() => activeSection = 'optimize'}>🧠 AI Optimize</button>
  </div>

  <!-- ═══════════════ TUNING SETTINGS ═══════════════ -->
  {#if activeSection === 'tuning'}
    {#if settingsLoading}
      <div class="loading-state">Loading settings...</div>
    {:else}
      {#if settingsError}
        <div class="settings-banner error">{settingsError}</div>
      {/if}
      {#if settingsSuccess}
        <div class="settings-banner success">{settingsSuccess}</div>
      {/if}

      <div class="tuning-grid">
        <!-- Smart Defaults Toggle -->
        <div class="tuning-card smart-defaults-card" title="Smart Defaults: When enabled, the server automatically adjusts inference settings (context size, GPU layers, KV cache, etc.) based on your hardware and model size to prevent OOM crashes. Disable this to use your own settings exactly as configured.">
          <div class="tc-header">
            <label class="tc-label" for="smart-defaults">Smart Defaults</label>
            <span class="tc-value" style="color: {smartDefaults ? '#6ee7b7' : '#fbbf24'}">{smartDefaults ? 'ON' : 'OFF'}</span>
          </div>
          <p class="tc-desc">
            {#if smartDefaults}
              Settings below are <strong>auto-tuned</strong> on model load based on your hardware. Your values act as hints that may be adjusted.
            {:else}
              Settings below are used <strong>exactly as configured</strong>. Make sure your hardware can handle them or the model may fail to load.
            {/if}
          </p>
          <label class="toggle-switch">
            <input type="checkbox" id="smart-defaults" bind:checked={smartDefaults} onchange={markDirty} />
            <span class="toggle-slider"></span>
            <span class="toggle-text">{smartDefaults ? 'Auto-tune enabled' : 'Manual mode'}</span>
          </label>
        </div>

        <!-- GPU Layers -->
        <div class="tuning-card" title="GPU Layers: Number of transformer layers offloaded to GPU VRAM. More layers = faster inference but more VRAM used. Set to -1 to offload all layers. Independent of model quantization. Impact: Increasing moves computation from CPU to GPU for faster speed; decreasing frees VRAM but slows inference. Requires model reload.">
          <div class="tc-header">
            <label class="tc-label" for="gpu-layers">GPU Layers</label>
            <span class="tc-value">{gpuLayers === -1 ? 'All' : gpuLayers}</span>
          </div>
          <p class="tc-desc">Layers offloaded to GPU (VRAM). Independent of model quantization — you can offload all layers of a Q4 model or none of an FP16 model.</p>
          <input type="range" id="gpu-layers" min="-1" max="200" step="1"
            bind:value={gpuLayers} oninput={markDirty} />
          <div class="tc-range-labels">
            <span>-1 (All)</span>
            <span>200</span>
          </div>
          <input type="number" class="tc-number" min="-1" max="9999"
            bind:value={gpuLayers} oninput={markDirty} />
        </div>

        <!-- Context Size -->
        <div class="tuning-card" title="Context Window: Maximum number of tokens the model can see at once (prompt + response). Larger values let the model handle longer conversations but use significantly more VRAM for the KV cache. Impact: Doubling context roughly doubles KV cache VRAM. Use KV cache quantization (q8_0 or lower) to offset. Requires model reload.">
          <div class="tc-header">
            <label class="tc-label" for="ctx-size">Context Window</label>
            <span class="tc-value">{contextSize.toLocaleString()} tokens</span>
          </div>
          <p class="tc-desc">Max token window. Larger contexts need more VRAM for the KV cache — adjust KV cache quantization below to compensate.</p>
          <input type="range" id="ctx-size" min="512" max="131072" step="512"
            bind:value={contextSize} oninput={markDirty} />
          <div class="tc-range-labels">
            <span>512</span>
            <span>131K</span>
          </div>
          <div class="tc-presets">
            {#each ctxPresets as preset}
              <button class="preset-btn" class:active={contextSize === preset}
                onclick={() => { contextSize = preset; markDirty(); }}>
                {preset >= 1024 ? `${preset / 1024}K` : preset}
              </button>
            {/each}
          </div>
        </div>

        <!-- Parallel Slots -->
        <div class="tuning-card" title="Parallel Slots: Number of concurrent inference requests the server can process simultaneously. Each slot maintains its own KV cache, multiplying VRAM usage. Impact: More slots let you serve multiple users at once but multiply VRAM consumption. For personal use, 1 is usually fine. Requires model reload.">
          <div class="tc-header">
            <label class="tc-label" for="par-slots">Parallel Slots</label>
            <span class="tc-value">{parallelSlots}</span>
          </div>
          <p class="tc-desc">Concurrent inference slots. More slots = serve more users but use more VRAM.</p>
          <input type="range" id="par-slots" min="1" max="16" step="1"
            bind:value={parallelSlots} oninput={markDirty} />
          <div class="tc-range-labels">
            <span>1</span>
            <span>16</span>
          </div>
          <input type="number" class="tc-number" min="1" max="16"
            bind:value={parallelSlots} oninput={markDirty} />
        </div>

        <!-- Threads -->
        <div class="tuning-card" title="CPU Threads: Number of CPU threads used during token generation. -1 lets llama.cpp auto-detect the optimal count. Impact: More threads can speed up CPU-bound generation but setting too high (beyond physical cores) causes contention and hurts performance. Usually best left at auto or set to physical core count. Applied live.">
          <div class="tc-header">
            <label class="tc-label" for="threads">CPU Threads</label>
            <span class="tc-value">{threads === -1 ? 'Auto' : threads}{cpuThreads ? ` / ${cpuThreads}` : ''}</span>
          </div>
          <p class="tc-desc">CPU threads for generation. -1 = auto. Detected: {cpuCores} cores / {cpuThreads} threads.</p>
          <input type="range" id="threads" min="-1" max={cpuThreads || 64} step="1"
            bind:value={threads} oninput={markDirty} />
          <div class="tc-range-labels">
            <span>-1 (Auto)</span>
            <span>{cpuThreads || 64}</span>
          </div>
          <input type="number" class="tc-number" min="-1" max={cpuThreads || 256}
            bind:value={threads} oninput={markDirty} />
        </div>

        <!-- Threads Batch -->
        <div class="tuning-card" title="Batch Threads: CPU threads used specifically for prompt processing (the 'prefill' phase). -1 means use the same as generation threads. Impact: Can be set higher than generation threads since prompt processing is more parallelizable. Setting to total thread count can speed up prompt eval without hurting generation. Applied live.">
          <div class="tc-header">
            <label class="tc-label" for="threads-batch">Batch Threads</label>
            <span class="tc-value">{threadsBatch === -1 ? 'Auto' : threadsBatch}{cpuThreads ? ` / ${cpuThreads}` : ''}</span>
          </div>
          <p class="tc-desc">CPU threads for prompt processing. -1 = same as generation threads.</p>
          <input type="range" id="threads-batch" min="-1" max={cpuThreads || 64} step="1"
            bind:value={threadsBatch} oninput={markDirty} />
          <div class="tc-range-labels">
            <span>-1 (Auto)</span>
            <span>{cpuThreads || 64}</span>
          </div>
          <input type="number" class="tc-number" min="-1" max={cpuThreads || 256}
            bind:value={threadsBatch} oninput={markDirty} />
        </div>

        <!-- Batch Size -->
        <div class="tuning-card" title="Batch Size (-b): Maximum number of tokens processed in a single logical batch during prompt evaluation. Higher values speed up how fast long prompts are processed but use more VRAM. Impact: Increasing improves prompt processing speed (time-to-first-token). 2048 is a good default. Requires model reload.">
          <div class="tc-header">
            <label class="tc-label" for="batch-size">Batch Size</label>
            <span class="tc-value">{batchSize}</span>
          </div>
          <p class="tc-desc">Logical batch size (-b) for prompt processing. Higher = faster prompt eval, more VRAM.</p>
          <input type="range" id="batch-size" min="32" max="16384" step="32"
            bind:value={batchSize} oninput={markDirty} />
          <div class="tc-range-labels">
            <span>32</span>
            <span>16K</span>
          </div>
          <div class="tc-presets">
            {#each [256, 512, 1024, 2048, 4096, 8192] as preset}
              <button class="preset-btn" class:active={batchSize === preset}
                onclick={() => { batchSize = preset; markDirty(); }}>
                {preset >= 1024 ? `${preset / 1024}K` : preset}
              </button>
            {/each}
          </div>
        </div>

        <!-- Ubatch Size -->
        <div class="tuning-card" title="Physical Batch (-ub): The actual number of tokens sent to the GPU in one compute pass. Must be ≤ Batch Size. Controls peak VRAM usage during prompt processing. Impact: Lower values reduce peak VRAM spikes but slow down prompt eval. 512 is a safe default. Increase if you have VRAM headroom and want faster prompt processing. Requires model reload.">
          <div class="tc-header">
            <label class="tc-label" for="ubatch-size">Physical Batch</label>
            <span class="tc-value">{ubatchSize}</span>
          </div>
          <p class="tc-desc">Physical batch size (-ub). Must be ≤ batch size. Controls actual GPU compute granularity and peak VRAM.</p>
          <input type="range" id="ubatch-size" min="32" max="8192" step="32"
            bind:value={ubatchSize} oninput={markDirty} />
          <div class="tc-range-labels">
            <span>32</span>
            <span>8K</span>
          </div>
          <div class="tc-presets">
            {#each [128, 256, 512, 1024, 2048] as preset}
              <button class="preset-btn" class:active={ubatchSize === preset}
                onclick={() => { ubatchSize = preset; markDirty(); }}>
                {preset >= 1024 ? `${preset / 1024}K` : preset}
              </button>
            {/each}
          </div>
        </div>

        <!-- KV Cache K -->
        <div class="tuning-card" title="KV Cache Keys (-ctk): Quantization level for the Key portion of the attention KV cache. Completely independent of model weight quantization. Impact: Lower precision (e.g. q4_0) saves significant VRAM especially with large contexts, with minor quality trade-off. q8_0 is the safe choice — saves ~50% vs f16 with negligible quality loss. Requires model reload.">
          <div class="tc-header">
            <label class="tc-label" for="kv-type-k">KV Cache Keys</label>
            <span class="tc-value">{kvCacheTypeK}</span>
          </div>
          <p class="tc-desc">Key cache quantization (-ctk). Independent of model weights. q8_0 saves ~50% VRAM vs f16.</p>
          <select id="kv-type-k" class="tc-dropdown" bind:value={kvCacheTypeK} onchange={markDirty}>
            {#each ['f32','f16','bf16','q8_0','q5_1','q5_0','q4_1','q4_0','iq4_nl'] as opt}
              <option value={opt}>{opt}</option>
            {/each}
          </select>
        </div>

        <!-- KV Cache V -->
        <div class="tuning-card" title="KV Cache Values (-ctv): Quantization level for the Value portion of the KV cache. Can be set more aggressively than Keys with less quality impact. Impact: You can quantize V lower than K (e.g. K=q8_0, V=q4_0) to save extra VRAM with almost no quality loss. Good for fitting larger contexts into limited VRAM. Requires model reload.">
          <div class="tc-header">
            <label class="tc-label" for="kv-type-v">KV Cache Values</label>
            <span class="tc-value">{kvCacheTypeV}</span>
          </div>
          <p class="tc-desc">Value cache quantization (-ctv). Can be lower than K with almost no quality loss.</p>
          <select id="kv-type-v" class="tc-dropdown" bind:value={kvCacheTypeV} onchange={markDirty}>
            {#each ['f32','f16','bf16','q8_0','q5_1','q5_0','q4_1','q4_0','iq4_nl'] as opt}
              <option value={opt}>{opt}</option>
            {/each}
          </select>
        </div>

        <!-- GPU Backend -->
        <div class="tuning-card" title="GPU Backend: Which GPU acceleration library to use. CUDA is fastest on NVIDIA GPUs, Vulkan is cross-platform, ROCm for AMD GPUs, CPU for no GPU acceleration. Impact: CUDA typically gives 10-30% better performance than Vulkan on NVIDIA. 'auto' picks the best available. Changing requires model reload with the corresponding llama-server binary.">
          <div class="tc-header">
            <label class="tc-label" for="gpu-backend">GPU Backend</label>
            <span class="tc-value">{gpuBackend}</span>
          </div>
          <p class="tc-desc">Hardware acceleration backend for inference.</p>
          <select id="gpu-backend" class="tc-dropdown" bind:value={gpuBackend} onchange={markDirty}>
            {#each ['auto', 'vulkan', 'cuda', 'rocm', 'cpu'] as opt}
              {@const isInstalled = opt === 'auto' || availableBackends.includes(opt)}
              <option value={opt}>{opt}{isInstalled ? ' ✓' : ''}</option>
            {/each}
          </select>
        </div>

        <!-- Multi-GPU -->
        <div class="tuning-card" title="Multi-GPU Split: How to distribute the model across multiple GPUs. 'layer' splits by transformer layer (most common), 'row' splits tensor rows (more balanced VRAM but slower). 'none' disables multi-GPU. Impact: Only relevant with 2+ GPUs. Layer split is easiest and most stable. Use Tensor Split to control the ratio (e.g. '3,1' gives 75%/25%). Requires model reload.">
          <div class="tc-header">
            <label class="tc-label" for="split-mode">Multi-GPU Split</label>
            <span class="tc-value">{splitMode}</span>
          </div>
          <p class="tc-desc">How to split the model across GPUs.</p>
          <select id="split-mode" class="tc-dropdown" bind:value={splitMode} onchange={markDirty}>
            <option value="none">none</option>
            <option value="layer">layer</option>
            <option value="row">row</option>
          </select>
          <div class="tc-inline-group">
            <div class="tc-inline-field">
              <label class="tc-sublabel" for="main-gpu">Main GPU</label>
              <input type="number" id="main-gpu" class="tc-number" min="0" max="15"
                bind:value={mainGpu} oninput={markDirty} />
            </div>
            <div class="tc-inline-field">
              <label class="tc-sublabel" for="tensor-split">Tensor Split</label>
              <input type="text" id="tensor-split" class="tc-text" placeholder="e.g. 3,1"
                bind:value={tensorSplit} oninput={markDirty} />
            </div>
          </div>
        </div>

        <!-- RoPE Settings -->
        <div class="tuning-card" title="RoPE Scaling: Method for extending the model's context length beyond its training length using Rotary Position Embeddings. 'linear' scales positions linearly, 'YaRN' uses a more sophisticated approach that preserves quality better at long contexts. Impact: Only needed when using context sizes larger than the model was trained for. Wrong settings can severely degrade quality. Leave at 'default' unless you know what you're doing. Requires model reload.">
          <div class="tc-header">
            <label class="tc-label" for="rope-scaling">RoPE Scaling</label>
            <span class="tc-value">{ropeScaling || 'default'}</span>
          </div>
          <p class="tc-desc">Rotary Position Embedding scaling for extended context.</p>
          <select id="rope-scaling" class="tc-dropdown" bind:value={ropeScaling} onchange={markDirty}>
            <option value="">default</option>
            <option value="none">none</option>
            <option value="linear">linear</option>
            <option value="yarn">YaRN</option>
          </select>
          <div class="tc-inline-group">
            <div class="tc-inline-field">
              <label class="tc-sublabel" for="rope-freq-base">Freq Base</label>
              <input type="number" id="rope-freq-base" class="tc-number" step="0.1" min="0"
                bind:value={ropeFreqBase} oninput={markDirty} />
            </div>
            <div class="tc-inline-field">
              <label class="tc-sublabel" for="rope-freq-scale">Freq Scale</label>
              <input type="number" id="rope-freq-scale" class="tc-number" step="0.01" min="0"
                bind:value={ropeFreqScale} oninput={markDirty} />
            </div>
          </div>
          <p class="tc-hint">0 = use model default. Only change if extending context beyond model training length.</p>
        </div>

        <!-- Max Predict -->
        <div class="tuning-card" title="Max Predict: Default maximum number of tokens the model will generate per request. -1 means unlimited (generate until stop token). Impact: Acts as a safety limit to prevent runaway generation. Individual API requests can override this. Lower values prevent excessively long responses. Applied live without reload.">
          <div class="tc-header">
            <label class="tc-label" for="n-predict">Max Predict</label>
            <span class="tc-value">{nPredict === -1 ? '∞' : nPredict}</span>
          </div>
          <p class="tc-desc">Default max tokens to generate per request. -1 = unlimited.</p>
          <input type="number" id="n-predict" class="tc-number" min="-1" max="131072"
            bind:value={nPredict} oninput={markDirty} />
        </div>

        <!-- Toggles -->
        <div class="tuning-card toggles-card">
          <div class="toggle-row" title="Flash Attention: Uses an optimized algorithm for attention computation that is both faster and uses less memory. Impact: Enabling gives 10-30% speed boost and reduces VRAM usage. Should almost always be ON. Only disable if you experience compatibility issues with specific models. Requires model reload.">
            <div class="toggle-info">
              <span class="tc-label">Flash Attention</span>
              <p class="tc-desc">Optimized attention computation. Faster and less memory.</p>
            </div>
            <button class="toggle-switch" class:on={flashAttention}
              onclick={() => { flashAttention = !flashAttention; markDirty(); }}
              role="switch" aria-checked={flashAttention} aria-label="Flash Attention">
              <span class="toggle-knob"></span>
            </button>
          </div>
          <div class="toggle-row" title="Continuous Batching: Allows the server to dynamically insert new requests into ongoing batches rather than waiting for all slots to finish. Impact: Dramatically improves throughput when serving multiple users. Should always be ON. Only disable for debugging. Requires model reload.">
            <div class="toggle-info">
              <span class="tc-label">Continuous Batching</span>
              <p class="tc-desc">Process multiple requests efficiently in a single batch.</p>
            </div>
            <button class="toggle-switch" class:on={continuousBatching}
              onclick={() => { continuousBatching = !continuousBatching; markDirty(); }}
              role="switch" aria-checked={continuousBatching} aria-label="Continuous Batching">
              <span class="toggle-knob"></span>
            </button>
          </div>
          <div class="toggle-row" title="Prompt Caching: Reuses the KV cache from previous requests when they share the same prompt prefix (like a system prompt). Impact: Significantly speeds up subsequent requests in a conversation by skipping redundant computation. Should be ON for chat use cases. Minimal VRAM overhead. Applied live.">
            <div class="toggle-info">
              <span class="tc-label">Prompt Caching</span>
              <p class="tc-desc">Reuse KV cache from previous requests for common prefixes.</p>
            </div>
            <button class="toggle-switch" class:on={cachePrompt}
              onclick={() => { cachePrompt = !cachePrompt; markDirty(); }}
              role="switch" aria-checked={cachePrompt} aria-label="Prompt Caching">
              <span class="toggle-knob"></span>
            </button>
          </div>
          <div class="toggle-row" title="Warmup: Runs a dummy inference pass immediately after loading the model to pre-initialize GPU caches and memory allocators. Impact: Makes the first real request faster (avoids cold-start latency) at the cost of slightly longer model load time. Recommended ON for interactive use. Requires model reload.">
            <div class="toggle-info">
              <span class="tc-label">Warmup</span>
              <p class="tc-desc">Run an empty warmup pass on model load for faster first inference.</p>
            </div>
            <button class="toggle-switch" class:on={warmup}
              onclick={() => { warmup = !warmup; markDirty(); }}
              role="switch" aria-checked={warmup} aria-label="Warmup">
              <span class="toggle-knob"></span>
            </button>
          </div>
          <div class="toggle-row" title="Memory Lock (mlock): Pins the model weights in physical RAM, preventing the OS from swapping them to disk. Impact: Prevents sudden slowdowns from disk swapping during inference, but locks that RAM from other applications. Useful on systems with enough RAM. May require elevated privileges on some systems. Requires model reload.">
            <div class="toggle-info">
              <span class="tc-label">Memory Lock</span>
              <p class="tc-desc">Force model to stay in RAM, preventing swap/compression.</p>
            </div>
            <button class="toggle-switch" class:on={mlock}
              onclick={() => { mlock = !mlock; markDirty(); }}
              role="switch" aria-checked={mlock} aria-label="Memory Lock">
              <span class="toggle-knob"></span>
            </button>
          </div>
          <div class="toggle-row" title="Disable mmap (no_mmap): Disables memory-mapped file loading, forcing the model to be fully read into RAM instead. Impact: Slower initial load but may prevent page faults and memory pressure on systems with limited RAM or heavy multitasking. Usually leave OFF unless you experience stuttering during inference. Requires model reload.">
            <div class="toggle-info">
              <span class="tc-label">Disable mmap</span>
              <p class="tc-desc">Disable memory-mapping. Slower load but may reduce pageouts.</p>
            </div>
            <button class="toggle-switch" class:on={noMmap}
              onclick={() => { noMmap = !noMmap; markDirty(); }}
              role="switch" aria-checked={noMmap} aria-label="Disable mmap">
              <span class="toggle-knob"></span>
            </button>
          </div>
        </div>
      </div>

      <!-- Apply Actions -->
      <div class="apply-bar">
        <div class="apply-options">
          <label class="apply-check">
            <input type="checkbox" bind:checked={reloadAfterApply} />
            <span>Reload model after apply</span>
          </label>
          <label class="apply-check">
            <input type="checkbox" bind:checked={saveToDisk} />
            <span>Save to config.toml</span>
          </label>
        </div>
        <div class="apply-buttons">
          <button class="btn-reset" onclick={resetSettings} disabled={!settingsDirty || settingsApplying}>
            Reset
          </button>
          <button class="btn-apply" onclick={applySettings} disabled={!settingsDirty || settingsApplying}>
            {settingsApplying ? 'Applying...' : `⚡ Apply${buildChanges().length > 0 ? ` (${buildChanges().length})` : ''}`}
          </button>
        </div>
      </div>
    {/if}

  <!-- ═══════════════ API LOGS ═══════════════ -->
  {:else if activeSection === 'logs'}
    <div class="toolbar">
      <div class="toolbar-left">
        <h3>🔧 API Request Log</h3>
        <span class="log-count">{logs.length} entries</span>
      </div>
      <div class="toolbar-right">
        <input
          type="text"
          bind:value={modelFilter}
          placeholder="Filter by model..."
          class="filter-input"
          oninput={refreshLogs}
        />
        <label class="auto-toggle">
          <input type="checkbox" bind:checked={autoRefresh} />
          <span>Auto-refresh</span>
        </label>
        <button onclick={refreshLogs} class="btn-sm btn-refresh">↻ Refresh</button>
        <button onclick={clearLogs} class="btn-sm btn-clear">Clear</button>
      </div>
    </div>

    {#if logsLoading}
      <div class="loading-state">Loading logs...</div>
    {:else if logs.length === 0}
      <div class="empty-state">
        <p>No API requests logged yet. Send a chat message or make an API call to see traffic here.</p>
      </div>
    {:else}
      <div class="log-list">
        <div class="log-header-row">
          <span class="col-time">Time</span>
          <span class="col-method">Method</span>
          <span class="col-path">Path</span>
          <span class="col-model">Model</span>
          <span class="col-status">Status</span>
          <span class="col-duration">Duration</span>
          <span class="col-tps">Tok/s</span>
          <span class="col-tokens">Tokens</span>
        </div>
        {#each logs as log}
          <button class="log-row" class:expanded={expandedEntry === log.id} onclick={() => toggleEntry(log.id)}>
            <span class="col-time">{formatTime(log.timestamp)}</span>
            <span class="col-method method-badge">{log.method}</span>
            <span class="col-path">{log.path}</span>
            <span class="col-model" title={log.model}>{log.model || '-'}</span>
            <span class="col-status" style="color: {statusColor(log.status_code)}">{log.status_code}</span>
            <span class="col-duration">{log.duration_ms}ms</span>
            <span class="col-tps" style="color: {tpsColor(log.tokens_per_second)}">{log.tokens_per_second > 0 ? log.tokens_per_second.toFixed(1) : '-'}</span>
            <span class="col-tokens">{log.prompt_tokens > 0 ? `${log.prompt_tokens}→${log.completion_tokens}` : '-'}</span>
          </button>
          {#if expandedEntry === log.id}
            <div class="log-detail">
              <div class="detail-grid">
                <div class="detail-section">
                  <h4>Request Summary</h4>
                  <p>{log.request_summary}</p>
                </div>
                <div class="detail-section">
                  <h4>Response Summary</h4>
                  <p>{log.response_summary}</p>
                </div>
              </div>
              {#if log.request_body && showRequestBody}
                <div class="detail-section">
                  <h4>Request Body <button class="toggle-btn" onclick={(e) => { e.stopPropagation(); showRequestBody = !showRequestBody; }}>Hide</button></h4>
                  <pre class="code-block">{formatJson(log.request_body)}</pre>
                </div>
              {/if}
              {#if log.response_body && showResponseBody}
                <div class="detail-section">
                  <h4>Response Preview <button class="toggle-btn" onclick={(e) => { e.stopPropagation(); showResponseBody = !showResponseBody; }}>Hide</button></h4>
                  <pre class="code-block response-block">{log.response_body}</pre>
                </div>
              {/if}
              <div class="detail-stats">
                <span>Prompt: <strong>{log.prompt_tokens}</strong> tok</span>
                <span>Completion: <strong>{log.completion_tokens}</strong> tok</span>
                <span>Duration: <strong>{log.duration_ms}</strong>ms</span>
                <span>Speed: <strong style="color: {tpsColor(log.tokens_per_second)}">{log.tokens_per_second.toFixed(1)}</strong> tok/s</span>
                {#if log.time_to_first_token_ms}
                  <span>TTFT: <strong>{log.time_to_first_token_ms}</strong>ms</span>
                {/if}
              </div>
            </div>
          {/if}
        {/each}
      </div>
    {/if}

  <!-- ═══════════════ AI OPTIMIZE ═══════════════ -->
  {:else if activeSection === 'optimize'}
    <AutoOptimize />
  {/if}
</div>

<style>
  .dev-panel { display: flex; flex-direction: column; gap: 0.5rem; }

  /* ── Section Tabs ── */
  .section-tabs {
    display: flex;
    gap: 0.25rem;
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    padding: 0.4rem;
  }
  .section-tabs button {
    flex: 1;
    background: transparent;
    border: 1px solid transparent;
    color: #666;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
    transition: all 0.15s;
  }
  .section-tabs button:hover { color: #aaa; background: #1a1a2e; }
  .section-tabs button.active {
    color: #6ee7b7;
    background: #1a1a2e;
    border-color: #2a2a40;
  }
  .tab-badge {
    font-size: 0.7rem;
    background: #2a2a40;
    padding: 0.1rem 0.4rem;
    border-radius: 10px;
    margin-left: 0.3rem;
    color: #a0a0b8;
  }
  .tuning-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 0.4rem;
  }
  .tuning-card {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    padding: 0.5rem 0.7rem;
  }
  .smart-defaults-card {
    grid-column: 1 / -1;
    border-color: #2a2a40;
    background: #0a0a16;
  }
  /* Toggle switch */
  .toggle-switch {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    user-select: none;
  }
  .toggle-switch input { display: none; }
  .toggle-slider {
    position: relative;
    width: 36px;
    height: 20px;
    background: #333;
    border-radius: 10px;
    transition: background 0.2s;
    flex-shrink: 0;
  }
  .toggle-slider::after {
    content: '';
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    background: #888;
    border-radius: 50%;
    transition: transform 0.2s, background 0.2s;
  }
  .toggle-switch input:checked + .toggle-slider {
    background: #1a3a2a;
  }
  .toggle-switch input:checked + .toggle-slider::after {
    transform: translateX(16px);
    background: #6ee7b7;
  }
  .toggle-text {
    font-size: 0.75rem;
    color: #9a9ab0;
  }
  .tc-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.15rem;
  }
  .tc-label {
    font-size: 0.8rem;
    font-weight: 600;
    color: #e0e0e8;
  }
  .tc-value {
    font-size: 0.8rem;
    font-weight: 700;
    color: #6ee7b7;
    font-family: 'JetBrains Mono', monospace;
  }
  .tc-desc {
    font-size: 0.7rem;
    color: #9a9ab0;
    margin-bottom: 0.35rem;
    line-height: 1.3;
  }

  /* Range sliders */
  input[type="range"] {
    width: 100%;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: #1e1e30;
    border-radius: 2px;
    outline: none;
    cursor: pointer;
    margin: 0.15rem 0;
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 14px;
    height: 14px;
    background: #6ee7b7;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 0 4px rgba(110, 231, 183, 0.4);
    transition: transform 0.1s;
  }
  input[type="range"]::-webkit-slider-thumb:hover {
    transform: scale(1.15);
  }
  input[type="range"]::-moz-range-thumb {
    width: 14px;
    height: 14px;
    background: #6ee7b7;
    border-radius: 50%;
    border: none;
    cursor: pointer;
    box-shadow: 0 0 4px rgba(110, 231, 183, 0.4);
  }

  .tc-range-labels {
    display: flex;
    justify-content: space-between;
    font-size: 0.65rem;
    color: #7a7a90;
    margin-top: 0.1rem;
    margin-bottom: 0.2rem;
  }

  .tc-number {
    width: 80px;
    background: #1a1a2e;
    border: 1px solid #2a2a40;
    border-radius: 5px;
    color: #e0e0e8;
    padding: 0.25rem 0.4rem;
    font-size: 0.78rem;
    font-family: 'JetBrains Mono', monospace;
    outline: none;
  }
  .tc-number:focus { border-color: #6ee7b7; }

  .tc-text {
    flex: 1;
    background: #1a1a2e;
    border: 1px solid #2a2a40;
    border-radius: 5px;
    color: #e0e0e8;
    padding: 0.25rem 0.4rem;
    font-size: 0.78rem;
    font-family: 'JetBrains Mono', monospace;
    outline: none;
  }
  .tc-text:focus { border-color: #6ee7b7; }
  .tc-text::placeholder { color: #666; }

  .tc-inline-group {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.3rem;
  }
  .tc-inline-field {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    flex: 1;
  }
  .tc-sublabel {
    font-size: 0.68rem;
    color: #9a9ab0;
    font-weight: 500;
  }
  .tc-hint {
    font-size: 0.65rem;
    color: #8a8a9f;
    margin-top: 0.2rem;
    font-style: italic;
  }
  .tc-dropdown {
    background: #1a1a2e;
    border: 1px solid #2a2a40;
    color: #e0e0e0;
    padding: 0.3rem 0.5rem;
    border-radius: 5px;
    font-size: 0.78rem;
    font-family: 'JetBrains Mono', monospace;
    cursor: pointer;
    width: 100%;
    appearance: none;
    -webkit-appearance: none;
    color-scheme: dark;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6'%3E%3Cpath d='M0 0l5 6 5-6z' fill='%23888'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.5rem center;
    padding-right: 1.5rem;
    transition: border-color 0.15s;
  }
  .tc-dropdown:hover { border-color: #444; }
  .tc-dropdown:focus { border-color: #6ee7b7; outline: none; }
  .tc-dropdown option {
    background: #0d0d1a;
    color: #e0e0e0;
  }

  .tc-presets {
    display: flex;
    gap: 0.2rem;
    flex-wrap: wrap;
    margin-top: 0.2rem;
  }
  .preset-btn {
    background: #1a1a2e;
    border: 1px solid #2a2a40;
    color: #a0a0b8;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.72rem;
    font-family: 'JetBrains Mono', monospace;
    transition: all 0.15s;
  }
  .preset-btn:hover { color: #ccc; border-color: #444; }
  .preset-btn.active {
    background: #1a3328;
    border-color: #6ee7b7;
    color: #6ee7b7;
  }

  /* Toggle switches */
  .toggles-card {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.4rem 0.6rem;
    grid-column: 1 / -1;
    background: transparent !important;
    border: none !important;
    padding: 0 !important;
  }
  .toggle-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 6px;
    padding: 0.4rem 0.6rem;
  }
  .toggle-info { flex: 1; }
  .toggle-info .tc-label { font-size: 0.75rem; }
  .toggle-info .tc-desc { margin-bottom: 0; font-size: 0.65rem; }

  .toggle-switch {
    width: 36px;
    height: 20px;
    background: #2a2a40;
    border: none;
    border-radius: 10px;
    cursor: pointer;
    position: relative;
    transition: background 0.2s;
    flex-shrink: 0;
  }
  .toggle-switch.on { background: #166534; }
  .toggle-knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    background: #888;
    border-radius: 50%;
    transition: all 0.2s;
  }
  .toggle-switch.on .toggle-knob {
    left: 18px;
    background: #6ee7b7;
  }

  /* Apply bar */
  .apply-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    flex-wrap: wrap;
    gap: 0.75rem;
  }
  .apply-options { display: flex; gap: 1.25rem; flex-wrap: wrap; }
  .apply-check {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.82rem;
    color: #888;
    cursor: pointer;
  }
  .apply-check input { accent-color: #22c55e; }
  .apply-buttons { display: flex; gap: 0.5rem; }

  .btn-reset {
    background: #1a1a2e;
    border: 1px solid #2a2a40;
    color: #888;
    padding: 0.5rem 1rem;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .btn-reset:hover { color: #ccc; border-color: #444; }
  .btn-reset:disabled { opacity: 0.3; cursor: not-allowed; }

  .btn-apply {
    background: linear-gradient(135deg, #166534 0%, #0f4c3a 100%);
    border: 1px solid #22c55e;
    color: #fff;
    padding: 0.5rem 1.25rem;
    border-radius: 8px;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }
  .btn-apply:hover { box-shadow: 0 4px 15px rgba(34, 197, 94, 0.25); }
  .btn-apply:disabled { opacity: 0.3; cursor: not-allowed; box-shadow: none; }

  .settings-banner {
    padding: 0.5rem 1rem;
    border-radius: 6px;
    font-size: 0.82rem;
  }
  .settings-banner.error {
    background: #331a1a;
    border: 1px solid #991b1b;
    color: #f87171;
  }
  .settings-banner.success {
    background: #0a2a1a;
    border: 1px solid #166534;
    color: #6ee7b7;
  }

  /* ── API Log Styles ── */
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .toolbar-left { display: flex; align-items: center; gap: 0.75rem; }
  .toolbar-left h3 {
    font-size: 0.9rem;
    color: #e0e0e8;
    text-transform: none;
    letter-spacing: 0;
  }
  .log-count { font-size: 0.8rem; color: #555; }
  .toolbar-right { display: flex; align-items: center; gap: 0.5rem; }

  .filter-input {
    padding: 0.3rem 0.6rem;
    border: 1px solid #2a2a40;
    border-radius: 4px;
    background: #080810;
    color: #e0e0e8;
    font-size: 0.8rem;
    width: 150px;
    outline: none;
  }
  .filter-input:focus { border-color: #6ee7b7; }

  .auto-toggle {
    display: flex; align-items: center; gap: 0.3rem;
    color: #888; font-size: 0.8rem; cursor: pointer;
  }
  .auto-toggle input { accent-color: #6ee7b7; }

  .btn-sm {
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    border: none;
    cursor: pointer;
    font-size: 0.75rem;
    font-weight: 500;
  }
  .btn-refresh { background: #1a1a2e; color: #888; }
  .btn-refresh:hover { background: #2a2a40; color: #ccc; }
  .btn-clear { background: #331a1a; color: #f87171; }
  .btn-clear:hover { background: #4a2020; }

  .loading-state, .empty-state {
    text-align: center; padding: 2rem; color: #555;
  }

  .log-list {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    overflow: hidden;
  }

  .log-header-row {
    display: grid;
    grid-template-columns: 70px 55px 1fr 1fr 50px 65px 55px 80px;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #555;
    border-bottom: 1px solid #1e1e30;
    background: #0a0a14;
  }

  .log-row {
    display: grid;
    grid-template-columns: 70px 55px 1fr 1fr 50px 65px 55px 80px;
    gap: 0.5rem;
    padding: 0.4rem 0.75rem;
    font-size: 0.82rem;
    border: none;
    border-bottom: 1px solid #111120;
    background: transparent;
    color: #c4c4d0;
    cursor: pointer;
    text-align: left;
    width: 100%;
    transition: background 0.1s;
  }
  .log-row:hover { background: #12121e; }
  .log-row.expanded { background: #0f1a1a; border-bottom-color: #1e3030; }

  .col-time { font-family: 'JetBrains Mono', monospace; color: #666; font-size: 0.78rem; }
  .col-method { font-size: 0.75rem; }
  .method-badge { color: #6ee7b7; font-weight: 600; }
  .col-path { font-family: 'JetBrains Mono', monospace; font-size: 0.78rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-model { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #888; }
  .col-status { font-weight: 600; font-size: 0.8rem; }
  .col-duration { color: #888; font-size: 0.8rem; }
  .col-tps { font-weight: 600; font-size: 0.8rem; }
  .col-tokens { font-size: 0.78rem; color: #888; white-space: nowrap; }

  .log-detail {
    padding: 0.75rem;
    background: #0a1215;
    border-bottom: 1px solid #1e3030;
  }

  .detail-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
    margin-bottom: 0.75rem;
  }

  .detail-section h4 {
    font-size: 0.75rem;
    text-transform: uppercase;
    color: #6ee7b7;
    margin-bottom: 0.3rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .detail-section p { font-size: 0.85rem; color: #aaa; }

  .toggle-btn {
    font-size: 0.65rem;
    background: #1a1a2e;
    border: none;
    color: #666;
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
    cursor: pointer;
  }

  .code-block {
    background: #080810;
    border: 1px solid #1e1e30;
    border-radius: 4px;
    padding: 0.5rem;
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
    font-size: 0.78rem;
    color: #a0a0b0;
    overflow-x: auto;
    max-height: 200px;
    overflow-y: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .response-block { color: #8ecae6; }

  .detail-stats {
    display: flex;
    gap: 1.25rem;
    padding-top: 0.5rem;
    margin-top: 0.5rem;
    border-top: 1px solid #1a2a2a;
    font-size: 0.8rem;
    color: #888;
    flex-wrap: wrap;
  }
  .detail-stats strong { color: #e0e0e8; }

  @media (max-width: 900px) {
    .log-header-row, .log-row {
      grid-template-columns: 60px 45px 1fr 70px 50px 50px;
    }
    .col-model, .col-tokens { display: none; }
    .detail-grid { grid-template-columns: 1fr; }
    .tuning-grid { grid-template-columns: 1fr; }
  }
</style>
