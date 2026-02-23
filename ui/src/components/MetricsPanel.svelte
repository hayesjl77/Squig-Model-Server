<script>
  let { metrics } = $props();
</script>

<div class="card">
  <h3>Performance Metrics</h3>
  {#if metrics}
    <div class="stat-row">
      <span class="label">Total Requests</span>
      <span class="value">{metrics.total_requests?.toLocaleString() || 0}</span>
    </div>
    <div class="stat-row">
      <span class="label">Active Requests</span>
      <span class="value active">{metrics.active_requests || 0}</span>
    </div>
    <div class="stat-row">
      <span class="label">Tokens Generated</span>
      <span class="value">{metrics.total_tokens_generated?.toLocaleString() || 0}</span>
    </div>
    <div class="stat-row">
      <span class="label">Prompt Tokens</span>
      <span class="value">{metrics.total_prompt_tokens?.toLocaleString() || 0}</span>
    </div>
    <div class="stat-row highlight">
      <span class="label">Avg Tokens/sec</span>
      <span class="value speed">{metrics.avg_tokens_per_second?.toFixed(1) || '0.0'}</span>
    </div>
    <div class="stat-row">
      <span class="label">Avg Latency</span>
      <span class="value">{metrics.avg_latency_ms?.toFixed(0) || '0'} ms</span>
    </div>
  {:else}
    <p class="empty">No data yet</p>
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
  .highlight { background: #0a1a15; margin: 0 -1.25rem; padding: 0.6rem 1.25rem; }
  .label { color: #888; font-size: 0.9rem; }
  .value { color: #e0e0e8; font-weight: 500; font-size: 0.9rem; }
  .speed { color: #6ee7b7; font-size: 1.1rem; font-weight: 700; }
  .active { color: #fbbf24; }

  .empty { color: #555; font-style: italic; }
</style>
