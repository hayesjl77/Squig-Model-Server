<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';

  let serverConf = $state(null);
  let loading = $state(true);
  let actionPending = $state('');
  let error = $state('');
  let pollTimer = $state(null);

  async function fetchConfig() {
    try {
      serverConf = await api.serverConfig();
      error = '';
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  async function stopInference() {
    actionPending = 'stopping';
    error = '';
    try {
      await api.unloadAll();
      await fetchConfig();
    } catch (e) {
      error = `Stop failed: ${e.message}`;
    } finally {
      actionPending = '';
    }
  }

  async function startInference() {
    if (!serverConf?.models?.default_model) {
      error = 'No default model set in config.toml. Load a model from the Models tab.';
      return;
    }
    actionPending = 'starting';
    error = '';
    try {
      await api.loadModel(serverConf.models.default_model);
      await fetchConfig();
    } catch (e) {
      error = `Start failed: ${e.message}`;
    } finally {
      actionPending = '';
    }
  }

  async function rescan() {
    actionPending = 'scanning';
    error = '';
    try {
      const res = await api.rescanModels();
      if (res.error) {
        error = res.error;
      }
      await fetchConfig();
    } catch (e) {
      error = `Rescan failed: ${e.message}`;
    } finally {
      actionPending = '';
    }
  }

  $effect(() => {
    // Poll server config every 5 seconds
    if (serverConf) {
      pollTimer = setInterval(fetchConfig, 5000);
      return () => clearInterval(pollTimer);
    }
  });

  onMount(() => {
    fetchConfig();
  });
</script>

<div class="card">
  <h3>Server Configuration</h3>
  {#if loading}
    <p class="empty">Loading...</p>
  {:else if serverConf}
    <div class="stat-row">
      <span class="label">Listen Address</span>
      <span class="value mono">{serverConf.server.host}:{serverConf.server.port}</span>
    </div>
    <div class="stat-row">
      <span class="label">Max Concurrent</span>
      <span class="value">{serverConf.server.max_concurrent_requests}</span>
    </div>
    <div class="stat-row">
      <span class="label">API Key</span>
      <span class="value">{serverConf.server.api_key_set ? '🔑 Set' : '🔓 None'}</span>
    </div>
    <div class="stat-row">
      <span class="label">Model Dirs</span>
      <span class="value dirs" title={serverConf.models.directories?.join('\n')}>
        {serverConf.models.directories?.length || 0} configured
      </span>
    </div>
    <div class="stat-row">
      <span class="label">Default Model</span>
      <span class="value truncate">{serverConf.models.default_model || 'None'}</span>
    </div>
    <div class="stat-row">
      <span class="label">Max Loaded</span>
      <span class="value">{serverConf.models.max_loaded_models}</span>
    </div>

    <!-- Inference Engine Status -->
    <div class="engine-section">
      <div class="engine-header">
        <span class="engine-label">Inference Engine</span>
        <span class="engine-status" class:running={serverConf.inference_engine.running}>
          {serverConf.inference_engine.running ? '● Running' : '○ Stopped'}
        </span>
      </div>

      {#if serverConf.inference_engine.running}
        <div class="engine-detail">
          <span class="detail-text">
            {serverConf.inference_engine.loaded_count} model{serverConf.inference_engine.loaded_count !== 1 ? 's' : ''} loaded
            ({serverConf.inference_engine.gpu_backend.toUpperCase()})
          </span>
        </div>
        <div class="loaded-list">
          {#each serverConf.inference_engine.loaded_models as m}
            <span class="loaded-chip">{m}</span>
          {/each}
        </div>
      {/if}

      <div class="engine-actions">
        {#if serverConf.inference_engine.running}
          <button class="btn-stop" onclick={stopInference} disabled={!!actionPending}>
            {actionPending === 'stopping' ? 'Stopping...' : '⏹ Stop Engine'}
          </button>
        {:else}
          <button class="btn-start" onclick={startInference} disabled={!!actionPending}>
            {actionPending === 'starting' ? 'Starting...' : '▶ Start Engine'}
          </button>
        {/if}
        <button class="btn-rescan" onclick={rescan} disabled={!!actionPending}>
          {actionPending === 'scanning' ? 'Scanning...' : '🔄 Rescan Models'}
        </button>
      </div>
    </div>

    {#if error}
      <div class="panel-error">{error}</div>
    {/if}

    <div class="config-hint">
      Edit <code>config.toml</code> to change server port, host, or model directories. Restart the app to apply.
    </div>
  {:else}
    <p class="empty">Could not load config</p>
  {/if}
</div>

<style>
  .card {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 10px;
    padding: 1.25rem;
  }
  h3 {
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #6ee7b7;
    margin-bottom: 1rem;
  }
  .stat-row {
    display: flex;
    justify-content: space-between;
    padding: 0.4rem 0;
    border-bottom: 1px solid #151525;
  }
  .label { color: #888; font-size: 0.85rem; }
  .value { color: #e0e0e8; font-weight: 500; font-size: 0.85rem; max-width: 60%; text-align: right; }
  .mono { font-family: 'Fira Code', 'Cascadia Code', monospace; color: #a78bfa; }
  .dirs { color: #6ee7b7; cursor: help; }
  .truncate {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
  }

  .engine-section {
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid #1e1e30;
  }
  .engine-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }
  .engine-label {
    font-size: 0.8rem;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .engine-status {
    font-size: 0.8rem;
    font-weight: 600;
    color: #666;
  }
  .engine-status.running { color: #22c55e; }

  .engine-detail {
    margin-bottom: 0.4rem;
  }
  .detail-text {
    font-size: 0.8rem;
    color: #aaa;
  }

  .loaded-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-bottom: 0.6rem;
  }
  .loaded-chip {
    background: #1a2e22;
    color: #6ee7b7;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 500;
  }

  .engine-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.5rem;
  }
  .engine-actions button {
    flex: 1;
    padding: 0.45rem 0.75rem;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 500;
    transition: all 0.15s;
  }
  .engine-actions button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn-stop {
    background: #3b1a1a;
    color: #f87171;
  }
  .btn-stop:hover:not(:disabled) { background: #4a2020; }
  .btn-start {
    background: #1a3328;
    color: #6ee7b7;
  }
  .btn-start:hover:not(:disabled) { background: #1e4030; }
  .btn-rescan {
    background: #1a1a2e;
    color: #a78bfa;
  }
  .btn-rescan:hover:not(:disabled) { background: #252540; }

  .panel-error {
    margin-top: 0.5rem;
    padding: 0.4rem 0.6rem;
    background: #331a1a;
    border: 1px solid #4a1a1a;
    border-radius: 4px;
    color: #f87171;
    font-size: 0.8rem;
  }

  .config-hint {
    margin-top: 0.75rem;
    padding-top: 0.5rem;
    border-top: 1px solid #151525;
    font-size: 0.75rem;
    color: #555;
  }
  .config-hint code {
    background: #1a1a2e;
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
    color: #6ee7b7;
    font-size: 0.75rem;
  }

  .empty { color: #555; font-style: italic; }
</style>
