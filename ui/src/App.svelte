<script>
  import { onMount } from 'svelte';
  import { api } from './lib/api.js';
  import ModelCard from './components/ModelCard.svelte';
  import HardwarePanel from './components/HardwarePanel.svelte';
  import ServerPanel from './components/ServerPanel.svelte';
  import MetricsPanel from './components/MetricsPanel.svelte';
  import ChatPanel from './components/ChatPanel.svelte';
  import HFSearchPanel from './components/HFSearchPanel.svelte';
  import DevPanel from './components/DevPanel.svelte';
  import PerfMonitor from './components/PerfMonitor.svelte';
  import HelpGuide from './components/HelpGuide.svelte';

  let status = $state(null);
  let availableModels = $state([]);
  let loadedModels = $state([]);
  let hardware = $state(null);
  let metrics = $state(null);
  let error = $state('');
  let activeTab = $state('dashboard');
  let loading = $state(true);

  async function refresh() {
    try {
      const [s, am, lm, hw, m] = await Promise.all([
        api.status(),
        api.availableModels(),
        api.loadedModels(),
        api.hardware(),
        api.metrics(),
      ]);
      status = s;
      availableModels = am.models || [];
      loadedModels = lm.models || [];
      hardware = hw;
      metrics = m;
      error = '';
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  async function loadModel(name) {
    try {
      await api.loadModel(name);
      await refresh();
    } catch (e) {
      error = `Failed to load model: ${e.message}`;
    }
  }

  async function unloadModel(name) {
    try {
      await api.unloadModel(name);
      await refresh();
    } catch (e) {
      error = `Failed to unload model: ${e.message}`;
    }
  }

  async function deleteModel(name) {
    try {
      await api.deleteModel(name);
      await refresh();
    } catch (e) {
      error = `Failed to delete model: ${e.message}`;
    }
  }

  function formatUptime(seconds) {
    if (!seconds) return '-';
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = seconds % 60;
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m ${s}s`;
    return `${s}s`;
  }

  onMount(() => {
    refresh();
    const interval = setInterval(refresh, 5000);
    return () => clearInterval(interval);
  });
</script>

<div class="app">
  <header>
    <div class="logo">
      <span class="bolt">⚡</span>
      <h1>Squig Model Server</h1>
    </div>
    <nav>
      <button class:active={activeTab === 'dashboard'} onclick={() => activeTab = 'dashboard'}>Dashboard</button>
      <button class:active={activeTab === 'models'} onclick={() => activeTab = 'models'}>Models</button>
      <button class:active={activeTab === 'huggingface'} onclick={() => activeTab = 'huggingface'}>🤗 HuggingFace</button>
      <button class:active={activeTab === 'chat'} onclick={() => activeTab = 'chat'}>Chat</button>
      <button class:active={activeTab === 'developer'} onclick={() => activeTab = 'developer'}>🔧 Developer</button>
      <button class:active={activeTab === 'performance'} onclick={() => activeTab = 'performance'}>📊 Performance</button>
      <button class:active={activeTab === 'help'} onclick={() => activeTab = 'help'}>📖 Guide</button>
    </nav>
    <div class="status-badge" class:online={status}>
      {status ? 'Online' : 'Connecting...'}
    </div>
  </header>

  {#if error}
    <div class="error-banner">{error}</div>
  {/if}

  <main>
    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
        <p>Connecting to server...</p>
      </div>
    {:else if activeTab === 'dashboard'}
      <div class="dashboard-grid">
        <div class="card stats-card">
          <h3>Server Status</h3>
          <div class="stat-row">
            <span class="label">Uptime</span>
            <span class="value">{formatUptime(status?.uptime_seconds)}</span>
          </div>
          <div class="stat-row">
            <span class="label">Loaded Models</span>
            <span class="value">{status?.loaded_models?.length || 0}</span>
          </div>
          <div class="stat-row">
            <span class="label">Available Models</span>
            <span class="value">{status?.available_models_count || 0}</span>
          </div>
          <div class="stat-row">
            <span class="label">Parallel Slots</span>
            <span class="value">{status?.config?.parallel_slots || '-'}</span>
          </div>
          <div class="stat-row">
            <span class="label">Context Size</span>
            <span class="value">{status?.config?.context_size?.toLocaleString() || '-'}</span>
          </div>
          <div class="stat-row">
            <span class="label">Flash Attention</span>
            <span class="value">{status?.config?.flash_attention ? '✅' : '❌'}</span>
          </div>
        </div>

        <HardwarePanel {hardware} />
        <ServerPanel />
        <MetricsPanel {metrics} />

        <div class="card loaded-models-card">
          <h3>Loaded Models</h3>
          {#if loadedModels.length === 0}
            <p class="empty">No models loaded. Go to Models tab to load one.</p>
          {:else}
            {#each loadedModels as model}
              <div class="loaded-model">
                <span class="model-name">{model.name}</span>
                <span class="model-port">:{model.port}</span>
                <button class="btn-sm btn-danger" onclick={() => unloadModel(model.name)}>Unload</button>
              </div>
            {/each}
          {/if}
        </div>
      </div>

    {:else if activeTab === 'models'}
      <div class="models-grid">
        {#if availableModels.length === 0}
          <div class="card empty-state">
            <h3>No Models Found</h3>
            <p>Place GGUF model files in your models directory, or update the paths in <code>config.toml</code>.</p>
          </div>
        {:else}
          {#each availableModels as model}
            <ModelCard
              {model}
              isLoaded={loadedModels.some(m => m.name === model.name)}
              onLoad={() => loadModel(model.name)}
              onUnload={() => unloadModel(model.name)}
              onDelete={() => deleteModel(model.name)}
            />
          {/each}
        {/if}
      </div>

    {:else if activeTab === 'huggingface'}
      <HFSearchPanel />

    {/if}

    <!-- Chat stays mounted so state persists across tab switches -->
    <div class="tab-persist" class:tab-hidden={activeTab !== 'chat'}>
      <ChatPanel {loadedModels} />
    </div>

    {#if activeTab === 'developer'}
      <DevPanel />

    {:else if activeTab === 'performance'}
      <PerfMonitor />

    {:else if activeTab === 'help'}
      <HelpGuide />
    {/if}
  </main>
</div>

<style>
  :global(*) { margin: 0; padding: 0; box-sizing: border-box; }
  :global(body) {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Inter', Roboto, sans-serif;
    background: #080810;
    color: #e0e0e8;
    line-height: 1.5;
  }
  :global(select) {
    background: #1a1a2e;
    color: #e0e0e8;
    border: 1px solid #2a2a40;
    -webkit-appearance: none;
    appearance: none;
  }
  :global(select option) {
    background: #1a1a2e;
    color: #e0e0e8;
  }

  .app { min-height: 100vh; }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1.5rem;
    background: #0e0e1a;
    border-bottom: 1px solid #1e1e30;
    position: sticky;
    top: 0;
    z-index: 100;
  }

  .logo { display: flex; align-items: center; gap: 0.5rem; }
  .logo h1 { font-size: 1.2rem; font-weight: 600; color: #fff; }
  .bolt { font-size: 1.4rem; }

  nav { display: flex; gap: 0.25rem; }
  nav button {
    background: transparent;
    border: 1px solid transparent;
    color: #888;
    padding: 0.4rem 1rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.9rem;
    transition: all 0.15s;
  }
  nav button:hover { color: #ccc; background: #1a1a2e; }
  nav button.active {
    color: #6ee7b7;
    background: #1a1a2e;
    border-color: #2a2a40;
  }

  .status-badge {
    padding: 0.3rem 0.8rem;
    border-radius: 20px;
    font-size: 0.8rem;
    font-weight: 500;
    background: #331a1a;
    color: #f87171;
  }
  .status-badge.online {
    background: #1a3328;
    color: #6ee7b7;
  }

  .error-banner {
    background: #331a1a;
    color: #f87171;
    padding: 0.75rem 1.5rem;
    font-size: 0.9rem;
    border-bottom: 1px solid #4a1a1a;
  }

  main { padding: 1.5rem; max-width: 1400px; margin: 0 auto; }

  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 60vh;
    gap: 1rem;
    color: #666;
  }
  .spinner {
    width: 32px; height: 32px;
    border: 3px solid #222;
    border-top-color: #6ee7b7;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .dashboard-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
    gap: 1rem;
  }

  .card {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 10px;
    padding: 1.25rem;
  }
  .card h3 {
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
  .stat-row:last-child { border-bottom: none; }
  .label { color: #888; font-size: 0.9rem; }
  .value { color: #e0e0e8; font-weight: 500; font-size: 0.9rem; }

  .loaded-model {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0;
    border-bottom: 1px solid #151525;
  }
  .model-name { flex: 1; font-weight: 500; }
  .model-port { color: #888; font-size: 0.85rem; }

  .models-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 1rem;
  }

  .empty { color: #555; font-style: italic; font-size: 0.9rem; }

  .btn-sm {
    padding: 0.25rem 0.6rem;
    border-radius: 4px;
    border: none;
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 500;
  }
  .btn-danger { background: #4a1a1a; color: #f87171; }
  .btn-danger:hover { background: #5a2020; }

  .tab-hidden { display: none; }

  .empty-state { text-align: center; padding: 3rem; }
  .empty-state p { color: #666; margin-top: 0.5rem; }
  .empty-state code { background: #1a1a2e; padding: 0.2rem 0.4rem; border-radius: 3px; color: #6ee7b7; }

  @media (max-width: 768px) {
    .dashboard-grid { grid-template-columns: 1fr; }
    nav button { padding: 0.3rem 0.6rem; font-size: 0.8rem; }
  }
</style>
