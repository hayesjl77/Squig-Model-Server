<script>
  import { api } from '../lib/api.js';

  let phase = $state('idle'); // idle | analyzing | results | applying | applied | error
  let settings = $state(null);
  let optimizeResult = $state(null);
  let applyResult = $state(null);
  let errorMsg = $state('');
  let selectedChanges = $state({});
  let reloadModel = $state(true);
  let saveToDisk = $state(false);
  let loadedModels = $state([]);

  async function loadSettings() {
    try {
      const [s, lm] = await Promise.all([
        api.devSettings(),
        api.loadedModels(),
      ]);
      settings = s;
      loadedModels = lm.models || [];
    } catch (e) {
      errorMsg = e.message;
    }
  }

  async function runOptimize() {
    phase = 'analyzing';
    errorMsg = '';
    optimizeResult = null;
    try {
      const res = await api.devOptimize();
      if (res.error) {
        errorMsg = res.error;
        phase = 'error';
        return;
      }
      optimizeResult = res;
      // Pre-select all changes
      const changes = res.parsed?.changes || [];
      const sel = {};
      changes.forEach((c, i) => sel[i] = true);
      selectedChanges = sel;
      phase = 'results';
    } catch (e) {
      errorMsg = e.message;
      phase = 'error';
    }
  }

  async function applyChanges() {
    phase = 'applying';
    errorMsg = '';

    const allChanges = optimizeResult?.parsed?.changes || [];
    const toApply = allChanges
      .filter((_, i) => selectedChanges[i])
      .map(c => ({
        setting: c.setting,
        value: parseValue(c.recommended_value),
      }));

    if (toApply.length === 0) {
      errorMsg = 'No changes selected';
      phase = 'results';
      return;
    }

    const modelName = loadedModels[0]?.name;

    try {
      const res = await api.devApplySettings(
        toApply,
        reloadModel && modelName ? modelName : undefined,
        saveToDisk,
      );
      applyResult = res;
      phase = 'applied';
      // Refresh settings
      await loadSettings();
    } catch (e) {
      errorMsg = e.message;
      phase = 'error';
    }
  }

  function parseValue(v) {
    if (v === 'true') return true;
    if (v === 'false') return false;
    if (typeof v === 'string' && /^-?\d+$/.test(v)) return parseInt(v, 10);
    if (typeof v === 'number') return v;
    if (typeof v === 'boolean') return v;
    return v;
  }

  function reset() {
    phase = 'idle';
    optimizeResult = null;
    applyResult = null;
    errorMsg = '';
    selectedChanges = {};
  }

  function impactColor(impact) {
    if (impact === 'high') return '#f87171';
    if (impact === 'medium') return '#fbbf24';
    return '#34d399';
  }

  function confidenceColor(c) {
    if (c === 'high') return '#6ee7b7';
    if (c === 'medium') return '#fbbf24';
    return '#f87171';
  }

  // Load settings on mount
  import { onMount } from 'svelte';
  onMount(loadSettings);
</script>

<div class="auto-optimize">
  <!-- Header -->
  <div class="opt-header">
    <div class="opt-title">
      <span class="opt-icon">🧠</span>
      <div>
        <h3>AI Self-Optimization</h3>
        <p class="opt-subtitle">Let the loaded model analyze its own performance and tune its settings</p>
      </div>
    </div>
  </div>

  {#if errorMsg}
    <div class="error-banner">{errorMsg}</div>
  {/if}

  <!-- Current Settings -->
  {#if settings}
    <div class="settings-grid">
      <div class="setting-chip">
        <span class="sc-label">GPU Layers</span>
        <span class="sc-value">{settings.gpu_layers === -1 ? 'All' : settings.gpu_layers}</span>
      </div>
      <div class="setting-chip">
        <span class="sc-label">Context</span>
        <span class="sc-value">{settings.context_size?.toLocaleString()}</span>
      </div>
      <div class="setting-chip">
        <span class="sc-label">Parallel Slots</span>
        <span class="sc-value">{settings.parallel_slots}</span>
      </div>
      <div class="setting-chip">
        <span class="sc-label">Flash Attn</span>
        <span class="sc-value">{settings.flash_attention ? '✅' : '❌'}</span>
      </div>
      <div class="setting-chip">
        <span class="sc-label">KV Cache K</span>
        <span class="sc-value">{settings.kv_cache_type_k}</span>
      </div>
      <div class="setting-chip">
        <span class="sc-label">KV Cache V</span>
        <span class="sc-value">{settings.kv_cache_type_v}</span>
      </div>
      <div class="setting-chip">
        <span class="sc-label">Backend</span>
        <span class="sc-value">{settings.gpu_backend}</span>
      </div>
    </div>
  {/if}

  <!-- Phase: Idle -->
  {#if phase === 'idle' || phase === 'error'}
    <div class="action-area">
      <button class="btn-optimize" onclick={runOptimize} disabled={loadedModels.length === 0}>
        🧠 Optimize Settings
      </button>
      {#if loadedModels.length === 0}
        <p class="hint">Load a model and run some requests first</p>
      {:else}
        <p class="hint">Using <strong>{loadedModels[0]?.name}</strong> to analyze performance</p>
      {/if}
    </div>
  {/if}

  <!-- Phase: Analyzing -->
  {#if phase === 'analyzing'}
    <div class="analyzing">
      <div class="pulse-ring"></div>
      <p>Model is analyzing its own performance...</p>
      <p class="hint">This may take 10-30 seconds depending on the model</p>
    </div>
  {/if}

  <!-- Phase: Results -->
  {#if phase === 'results' && optimizeResult?.parsed}
    {@const parsed = optimizeResult.parsed}
    {@const changes = parsed.changes || []}

    <!-- Analysis -->
    <div class="analysis-card">
      <h4>Model Analysis</h4>
      <p>{parsed.analysis}</p>
      <div class="meta-row">
        <span class="confidence" style="color:{confidenceColor(parsed.confidence)}">
          Confidence: <strong>{parsed.confidence?.toUpperCase()}</strong>
        </span>
        {#if parsed.expected_improvement}
          <span class="expected">Expected: {parsed.expected_improvement}</span>
        {/if}
      </div>
    </div>

    <!-- Warnings -->
    {#if parsed.warnings?.length > 0}
      <div class="warnings">
        {#each parsed.warnings as w}
          <div class="warning-item">⚠️ {w}</div>
        {/each}
      </div>
    {/if}

    <!-- Changes -->
    {#if changes.length > 0}
      <div class="changes-section">
        <h4>Recommended Changes</h4>
        {#each changes as change, idx}
          <label class="change-card" class:selected={selectedChanges[idx]}>
            <input type="checkbox" bind:checked={selectedChanges[idx]} />
            <div class="change-content">
              <div class="change-header">
                <span class="change-setting">{change.setting}</span>
                <span class="change-impact" style="color:{impactColor(change.impact)}">{change.impact?.toUpperCase()}</span>
              </div>
              <div class="change-values">
                <span class="old-val">{JSON.stringify(change.current_value)}</span>
                <span class="arrow">→</span>
                <span class="new-val">{JSON.stringify(change.recommended_value)}</span>
              </div>
              <p class="change-reason">{change.reason}</p>
            </div>
          </label>
        {/each}
      </div>

      <div class="apply-options">
        <label class="opt-check">
          <input type="checkbox" bind:checked={reloadModel} />
          <span>Reload model with new settings</span>
        </label>
        <label class="opt-check">
          <input type="checkbox" bind:checked={saveToDisk} />
          <span>Save to config.toml (persist across restarts)</span>
        </label>
      </div>

      <div class="apply-actions">
        <button class="btn-apply" onclick={applyChanges}>
          ⚡ Apply {Object.values(selectedChanges).filter(Boolean).length} Change(s)
        </button>
        <button class="btn-cancel" onclick={reset}>Cancel</button>
      </div>
    {:else}
      <div class="no-changes">
        <p>✅ The model determined that current settings are already optimal!</p>
      </div>
      <div class="apply-actions">
        <button class="btn-cancel" onclick={reset}>Done</button>
      </div>
    {/if}
  {/if}

  <!-- Phase: Applying -->
  {#if phase === 'applying'}
    <div class="analyzing">
      <div class="pulse-ring"></div>
      <p>Applying settings and reloading model...</p>
    </div>
  {/if}

  <!-- Phase: Applied -->
  {#if phase === 'applied' && applyResult}
    <div class="applied-card">
      <h4>✅ Settings Applied</h4>
      {#if applyResult.applied?.length > 0}
        <div class="applied-list">
          {#each applyResult.applied as item}
            <div class="applied-item">✓ {item}</div>
          {/each}
        </div>
      {/if}
      {#if applyResult.errors?.length > 0}
        <div class="applied-errors">
          {#each applyResult.errors as item}
            <div class="applied-error">✗ {item}</div>
          {/each}
        </div>
      {/if}
      {#if applyResult.reload_status}
        <p class="reload-status">Model reload: <strong>{applyResult.reload_status}</strong></p>
      {/if}
      <p class="hint">Run more requests and check the Performance tab to measure improvement.</p>
    </div>
    <div class="apply-actions">
      <button class="btn-optimize" onclick={reset}>🧠 Run Again</button>
    </div>
  {/if}
</div>

<style>
  .auto-optimize { display: flex; flex-direction: column; gap: 0.75rem; }

  .opt-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .opt-title { display: flex; align-items: center; gap: 0.75rem; }
  .opt-icon { font-size: 1.8rem; }
  .opt-title h3 {
    font-size: 1rem;
    color: #e0e0e8;
    text-transform: none;
    letter-spacing: 0;
  }
  .opt-subtitle { font-size: 0.82rem; color: #666; }

  .error-banner {
    background: #331a1a;
    border: 1px solid #991b1b;
    border-radius: 8px;
    padding: 0.6rem 1rem;
    color: #f87171;
    font-size: 0.85rem;
  }

  .settings-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .setting-chip {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 6px;
    padding: 0.4rem 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 100px;
  }
  .sc-label { font-size: 0.7rem; color: #555; text-transform: uppercase; letter-spacing: 0.03em; }
  .sc-value { font-size: 0.95rem; font-weight: 600; color: #e0e0e8; }

  .action-area { text-align: center; padding: 1.5rem; }
  .hint { font-size: 0.8rem; color: #555; margin-top: 0.5rem; }
  .hint strong { color: #6ee7b7; }

  .btn-optimize {
    background: linear-gradient(135deg, #166534 0%, #0f4c3a 100%);
    border: 1px solid #22c55e;
    color: #fff;
    padding: 0.75rem 2rem;
    border-radius: 10px;
    font-size: 1rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }
  .btn-optimize:hover { transform: translateY(-1px); box-shadow: 0 4px 20px rgba(34,197,94,0.3); }
  .btn-optimize:disabled { opacity: 0.4; cursor: not-allowed; transform: none; box-shadow: none; }

  .analyzing {
    text-align: center;
    padding: 2.5rem;
    color: #888;
  }
  .analyzing p { margin-top: 0.5rem; }
  .pulse-ring {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    border: 3px solid #22c55e;
    margin: 0 auto;
    animation: pulse 1.5s ease-in-out infinite;
  }
  @keyframes pulse {
    0%, 100% { transform: scale(1); opacity: 1; }
    50% { transform: scale(1.2); opacity: 0.5; }
  }

  .analysis-card {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    padding: 1rem;
  }
  .analysis-card h4 { font-size: 0.8rem; color: #6ee7b7; text-transform: uppercase; margin-bottom: 0.5rem; }
  .analysis-card p { font-size: 0.88rem; color: #aaa; line-height: 1.5; }
  .meta-row { display: flex; gap: 1.5rem; margin-top: 0.5rem; font-size: 0.82rem; flex-wrap: wrap; }
  .confidence strong { font-weight: 700; }
  .expected { color: #888; }

  .warnings { display: flex; flex-direction: column; gap: 0.3rem; }
  .warning-item {
    background: #2a2a0a;
    border: 1px solid #854d0e;
    border-radius: 6px;
    padding: 0.4rem 0.75rem;
    font-size: 0.82rem;
    color: #fbbf24;
  }

  .changes-section h4 { font-size: 0.8rem; color: #6ee7b7; text-transform: uppercase; margin-bottom: 0.5rem; }
  .change-card {
    display: flex;
    gap: 0.75rem;
    align-items: flex-start;
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    padding: 0.75rem 1rem;
    margin-bottom: 0.5rem;
    cursor: pointer;
    transition: border-color 0.15s;
  }
  .change-card:hover { border-color: #2a2a40; }
  .change-card.selected { border-color: #166534; background: #0a1a14; }
  .change-card input[type="checkbox"] { accent-color: #22c55e; margin-top: 0.2rem; }
  .change-content { flex: 1; }
  .change-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.3rem; }
  .change-setting { font-family: 'JetBrains Mono', monospace; font-size: 0.88rem; font-weight: 600; color: #e0e0e8; }
  .change-impact { font-size: 0.7rem; font-weight: 700; letter-spacing: 0.04em; }
  .change-values { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.3rem; }
  .old-val {
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.82rem;
    color: #f87171;
    text-decoration: line-through;
    opacity: 0.7;
  }
  .arrow { color: #555; }
  .new-val {
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.82rem;
    color: #6ee7b7;
    font-weight: 600;
  }
  .change-reason { font-size: 0.8rem; color: #888; line-height: 1.4; }

  .apply-options { display: flex; gap: 1.5rem; flex-wrap: wrap; }
  .opt-check {
    display: flex; align-items: center; gap: 0.4rem;
    font-size: 0.82rem; color: #888; cursor: pointer;
  }
  .opt-check input { accent-color: #22c55e; }

  .apply-actions { display: flex; gap: 0.75rem; justify-content: center; padding-top: 0.5rem; }
  .btn-apply {
    background: linear-gradient(135deg, #166534 0%, #0f4c3a 100%);
    border: 1px solid #22c55e;
    color: #fff;
    padding: 0.6rem 1.5rem;
    border-radius: 8px;
    font-size: 0.9rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }
  .btn-apply:hover { box-shadow: 0 4px 15px rgba(34,197,94,0.25); }
  .btn-cancel {
    background: #1a1a2e;
    border: 1px solid #2a2a40;
    color: #888;
    padding: 0.6rem 1.5rem;
    border-radius: 8px;
    font-size: 0.9rem;
    cursor: pointer;
  }
  .btn-cancel:hover { color: #ccc; border-color: #444; }

  .no-changes {
    background: #0a2a1a;
    border: 1px solid #166534;
    border-radius: 8px;
    padding: 1rem;
    text-align: center;
    color: #6ee7b7;
    font-size: 0.9rem;
  }

  .applied-card {
    background: #0a2a1a;
    border: 1px solid #166534;
    border-radius: 8px;
    padding: 1rem;
  }
  .applied-card h4 { color: #6ee7b7; margin-bottom: 0.5rem; }
  .applied-list { margin-bottom: 0.5rem; }
  .applied-item { font-size: 0.85rem; color: #6ee7b7; padding: 0.15rem 0; }
  .applied-errors { margin-bottom: 0.5rem; }
  .applied-error { font-size: 0.85rem; color: #f87171; padding: 0.15rem 0; }
  .reload-status { font-size: 0.85rem; color: #888; }
  .reload-status strong { color: #6ee7b7; }

  @media (max-width: 700px) {
    .settings-grid { gap: 0.3rem; }
    .setting-chip { min-width: 80px; padding: 0.3rem 0.5rem; }
  }
</style>
