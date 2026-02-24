<script>
  let { model, isLoaded, onLoad, onUnload, onDelete } = $props();
  let loading = $state(false);
  let confirmDelete = $state(false);

  // Check if this is an incomplete split model
  let isSplit = $derived(model.split_info != null);
  let isIncomplete = $derived(isSplit && !model.split_info.complete);

  async function handleAction() {
    if (isIncomplete) return; // Don't allow loading incomplete splits
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

  async function handleDelete() {
    if (!confirmDelete) {
      confirmDelete = true;
      setTimeout(() => confirmDelete = false, 3000);
      return;
    }
    loading = true;
    try {
      await onDelete();
    } finally {
      loading = false;
      confirmDelete = false;
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
    {#if isSplit}
      <div class="meta-row">
        <span class="meta-label">Parts</span>
        <span class="meta-value" class:split-incomplete={isIncomplete} class:split-complete={!isIncomplete}>
          {model.split_info.present_parts} / {model.split_info.total_parts}
          {isIncomplete ? ' ⚠️ Incomplete' : ' ✓'}
        </span>
      </div>
    {/if}
  </div>

  {#if isIncomplete}
    <div class="split-warning">
      ⚠️ Missing {model.split_info.total_parts - model.split_info.present_parts} of {model.split_info.total_parts} parts — download all parts to load this model
    </div>
  {/if}

  <button
    class="action-btn"
    class:unload={isLoaded}
    class:disabled-split={isIncomplete}
    disabled={loading || isIncomplete}
    onclick={handleAction}
  >
    {#if loading}
      <span class="btn-spinner"></span>
      {isLoaded ? 'Unloading...' : 'Loading...'}
    {:else}
      {isLoaded ? 'Unload Model' : 'Load Model'}
    {/if}
  </button>

  <button
    class="delete-btn"
    class:confirm={confirmDelete}
    disabled={loading}
    onclick={handleDelete}
    title="Delete model file from disk"
  >
    {confirmDelete ? '⚠️ Click again to confirm delete' : '🗑️ Delete'}
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
  .split-incomplete { color: #f59e0b; }
  .split-complete { color: #6ee7b7; }

  .split-warning {
    background: #2e2a1a;
    border: 1px solid #5a4a1a;
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    font-size: 0.78rem;
    color: #f59e0b;
    margin-bottom: 0.75rem;
    line-height: 1.4;
  }

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
  .action-btn.disabled-split { opacity: 0.3; cursor: not-allowed; background: #1a1a2a; color: #555; }
  .action-btn.unload { background: #2e1a1a; color: #f87171; }
  .action-btn.unload:hover { background: #3e2020; }

  .btn-spinner {
    width: 14px; height: 14px;
    border: 2px solid #333;
    border-top-color: #6ee7b7;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  .delete-btn {
    width: 100%;
    padding: 0.45rem;
    border-radius: 6px;
    border: 1px solid #2a1a1a;
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 500;
    background: transparent;
    color: #666;
    transition: all 0.15s;
    margin-top: 0.4rem;
  }
  .delete-btn:hover { background: #1a0e0e; color: #f87171; border-color: #3a1a1a; }
  .delete-btn:disabled { opacity: 0.4; cursor: wait; }
  .delete-btn.confirm { background: #2e1a1a; color: #f87171; border-color: #5a2020; animation: pulse-warn 1s ease-in-out infinite; }

  @keyframes pulse-warn {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.7; }
  }

  @keyframes spin { to { transform: rotate(360deg); } }
</style>
