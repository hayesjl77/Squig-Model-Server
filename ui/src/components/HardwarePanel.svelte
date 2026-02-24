<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';

  let { hardware } = $props();

  let gpuStats = $state([]);
  let pollTimer = $state(null);

  async function fetchGpuStats() {
    try {
      const res = await api.gpuStats();
      gpuStats = res.gpus || [];
    } catch (_) {}
  }

  function barColor(pct) {
    if (pct > 90) return '#ef4444';
    if (pct > 70) return '#f59e0b';
    if (pct > 40) return '#6ee7b7';
    return '#22c55e';
  }

  function tempColor(c) {
    if (c >= 85) return '#ef4444';
    if (c >= 70) return '#f59e0b';
    if (c >= 50) return '#6ee7b7';
    return '#22c55e';
  }

  onMount(() => {
    fetchGpuStats();
    pollTimer = setInterval(fetchGpuStats, 2000);
    return () => { if (pollTimer) clearInterval(pollTimer); };
  });
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
      <span class="value backend">{(hardware.active_backend || hardware.recommended_backend || 'unknown').toUpperCase()}</span>
    </div>
    {#if hardware.available_backends?.length > 0}
      <div class="stat-row">
        <span class="label">Compiled</span>
        <span class="value compiled-backends">
          {hardware.available_backends.map(b => b.toUpperCase()).join(' · ')}
        </span>
      </div>
    {/if}

    <!-- Live GPU Stats -->
    {#each gpuStats as gpu, i}
      <div class="gpu-live-section">
        <div class="gpu-header">
          <span class="gpu-name">{gpu.name}</span>
          {#if gpu.temperature_c != null}
            <span class="gpu-temp" style="color: {tempColor(gpu.temperature_c)}">{gpu.temperature_c}°C</span>
          {/if}
        </div>

        <!-- GPU Utilization Bar -->
        <div class="bar-row">
          <span class="bar-label">GPU</span>
          <div class="bar-track">
            <div class="bar-fill" style="width: {gpu.gpu_utilization_pct}%; background: {barColor(gpu.gpu_utilization_pct)}"></div>
          </div>
          <span class="bar-value">{gpu.gpu_utilization_pct.toFixed(0)}%</span>
        </div>

        <!-- VRAM Usage Bar -->
        <div class="bar-row">
          <span class="bar-label">VRAM</span>
          <div class="bar-track">
            <div class="bar-fill" style="width: {gpu.memory_utilization_pct}%; background: {barColor(gpu.memory_utilization_pct)}"></div>
          </div>
          <span class="bar-value">{(gpu.memory_used_mb / 1024).toFixed(1)} / {(gpu.memory_total_mb / 1024).toFixed(0)} GB</span>
        </div>

        <!-- Power Draw Bar (if available) -->
        {#if gpu.power_draw_w != null && gpu.power_limit_w != null}
          {@const powerPct = (gpu.power_draw_w / gpu.power_limit_w) * 100}
          <div class="bar-row">
            <span class="bar-label">Power</span>
            <div class="bar-track">
              <div class="bar-fill" style="width: {powerPct}%; background: {barColor(powerPct)}"></div>
            </div>
            <span class="bar-value">{gpu.power_draw_w.toFixed(0)}W / {gpu.power_limit_w.toFixed(0)}W</span>
          </div>
        {/if}

        <!-- Clocks & Fan (compact row) -->
        <div class="gpu-meta">
          {#if gpu.clock_graphics_mhz != null}
            <span class="meta-item">🔧 {gpu.clock_graphics_mhz} MHz</span>
          {/if}
          {#if gpu.clock_memory_mhz != null}
            <span class="meta-item">💾 {gpu.clock_memory_mhz} MHz</span>
          {/if}
          {#if gpu.fan_speed_pct != null}
            <span class="meta-item">🌀 Fan {gpu.fan_speed_pct}%</span>
          {/if}
        </div>
      </div>
    {/each}

    <!-- Fallback if no live stats but we know GPUs exist -->
    {#if gpuStats.length === 0 && hardware.gpus?.length > 0}
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
  .compiled-backends { color: #22c55e; font-size: 0.8rem; }

  /* GPU Live Stats */
  .gpu-live-section {
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid #1e1e30;
  }
  .gpu-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.6rem;
  }
  .gpu-name { color: #ccc; font-weight: 500; font-size: 0.85rem; }
  .gpu-temp { font-size: 0.85rem; font-weight: 600; }

  .bar-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.4rem;
  }
  .bar-label {
    font-size: 0.75rem;
    color: #666;
    width: 40px;
    flex-shrink: 0;
  }
  .bar-track {
    flex: 1;
    height: 10px;
    background: #151525;
    border-radius: 5px;
    overflow: hidden;
  }
  .bar-fill {
    height: 100%;
    border-radius: 5px;
    transition: width 0.5s ease, background 0.5s ease;
    min-width: 2px;
  }
  .bar-value {
    font-size: 0.7rem;
    color: #aaa;
    min-width: 90px;
    text-align: right;
    white-space: nowrap;
  }

  .gpu-meta {
    display: flex;
    gap: 0.75rem;
    margin-top: 0.35rem;
    flex-wrap: wrap;
  }
  .meta-item {
    font-size: 0.7rem;
    color: #666;
  }

  /* Fallback static GPU section */
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
  .gpu-vram { color: #6ee7b7; }
  .empty { color: #555; font-style: italic; }
</style>
