<script>
  let { hardware } = $props();
</script>

<div class="card">
  <h3>Hardware</h3>
  {#if hardware}
    <div class="stat-row">
      <span class="label">CPU</span>
      <span class="value">{hardware.cpu_name}</span>
    </div>
    <div class="stat-row">
      <span class="label">Cores / Threads</span>
      <span class="value">{hardware.cpu_cores} / {hardware.cpu_threads}</span>
    </div>
    <div class="stat-row">
      <span class="label">Total Memory</span>
      <span class="value">{hardware.total_memory_gb?.toFixed(1)} GB</span>
    </div>
    <div class="stat-row">
      <span class="label">Available Memory</span>
      <span class="value avail">{hardware.available_memory_gb?.toFixed(1)} GB</span>
    </div>
    <div class="stat-row">
      <span class="label">Backend</span>
      <span class="value backend">{hardware.recommended_backend?.toUpperCase()}</span>
    </div>
    {#if hardware.gpus?.length > 0}
      <div class="gpu-section">
        <span class="gpu-label">GPUs</span>
        {#each hardware.gpus as gpu}
          <div class="gpu-item">
            <span class="gpu-name">{gpu.name}</span>
            {#if gpu.vram_mb}
              <span class="gpu-vram">{(gpu.vram_mb / 1024).toFixed(0)} GB</span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {:else}
    <p class="empty">Detecting...</p>
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
  .stat-row:last-child { border-bottom: none; }
  .label { color: #888; font-size: 0.85rem; }
  .value { color: #e0e0e8; font-weight: 500; font-size: 0.85rem; max-width: 60%; text-align: right; }
  .avail { color: #6ee7b7; }
  .backend { color: #a78bfa; }

  .gpu-section {
    margin-top: 0.75rem;
    padding-top: 0.5rem;
    border-top: 1px solid #1e1e30;
  }
  .gpu-label { font-size: 0.8rem; color: #666; }
  .gpu-item {
    display: flex;
    justify-content: space-between;
    padding: 0.3rem 0;
    font-size: 0.85rem;
  }
  .gpu-name { color: #ccc; }
  .gpu-vram { color: #6ee7b7; }

  .empty { color: #555; font-style: italic; }
</style>
