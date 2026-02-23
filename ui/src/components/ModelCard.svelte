<script>
  let { model, isLoaded, onLoad, onUnload } = $props();
  let loading = $state(false);

  async function handleAction() {
    loading = true;
    try {
      if (isLoaded) {
        await onUnload();
      } else {
        await onLoad();
      }
    } finally {
      loading = false;
    }
  }
</script>

<div class="model-card" class:loaded={isLoaded}>
  <div class="header">
    <h4>{model.name}</h4>
    {#if isLoaded}
      <span class="badge loaded-badge">Loaded</span>
    {/if}
  </div>

  <div class="meta">
    <div class="meta-row">
      <span class="meta-label">Family</span>
      <span class="meta-value">{model.family}</span>
    </div>
    <div class="meta-row">
      <span class="meta-label">Parameters</span>
      <span class="meta-value">{model.parameters}</span>
    </div>
    <div class="meta-row">
      <span class="meta-label">Quantization</span>
      <span class="meta-value quant">{model.quantization}</span>
    </div>
    <div class="meta-row">
      <span class="meta-label">Size</span>
      <span class="meta-value">{model.size_human}</span>
    </div>
  </div>

  <button
    class="action-btn"
    class:unload={isLoaded}
    disabled={loading}
    onclick={handleAction}
  >
    {#if loading}
      <span class="btn-spinner"></span>
      {isLoaded ? 'Unloading...' : 'Loading...'}
    {:else}
      {isLoaded ? 'Unload Model' : 'Load Model'}
    {/if}
  </button>
</div>

<style>
  .model-card {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 10px;
    padding: 1.25rem;
    transition: border-color 0.2s;
  }
  .model-card:hover { border-color: #2a2a45; }
  .model-card.loaded { border-color: #2a4a3a; }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
  }
  h4 {
    font-size: 0.95rem;
    font-weight: 600;
    color: #fff;
    word-break: break-all;
  }

  .badge {
    padding: 0.15rem 0.5rem;
    border-radius: 12px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
  }
  .loaded-badge { background: #1a3328; color: #6ee7b7; }

  .meta { margin-bottom: 1rem; }
  .meta-row {
    display: flex;
    justify-content: space-between;
    padding: 0.3rem 0;
    font-size: 0.85rem;
  }
  .meta-label { color: #666; }
  .meta-value { color: #bbb; font-weight: 500; }
  .quant { color: #a78bfa; }

  .action-btn {
    width: 100%;
    padding: 0.6rem;
    border-radius: 6px;
    border: none;
    cursor: pointer;
    font-size: 0.9rem;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    background: #1a2e28;
    color: #6ee7b7;
    transition: background 0.15s;
  }
  .action-btn:hover { background: #1a3a30; }
  .action-btn:disabled { opacity: 0.5; cursor: wait; }
  .action-btn.unload { background: #2e1a1a; color: #f87171; }
  .action-btn.unload:hover { background: #3e2020; }

  .btn-spinner {
    width: 14px; height: 14px;
    border: 2px solid #333;
    border-top-color: #6ee7b7;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
