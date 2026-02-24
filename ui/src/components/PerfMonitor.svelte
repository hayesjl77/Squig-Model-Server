<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';
  import AutoOptimize from './AutoOptimize.svelte';

  let analysis = $state(null);
  let samples = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let autoRefresh = $state(true);

  async function refresh() {
    try {
      const [a, s] = await Promise.all([
        api.devPerf(),
        api.devPerfSamples(),
      ]);
      analysis = a;
      samples = s.samples || [];
      error = null;
    } catch (e) {
      console.error('PerfMonitor fetch error:', e);
      error = e.message || 'Failed to fetch performance data';
    }
    loading = false;
  }

  function ratingStyle(r) {
    const map = {
      excellent: { bg: '#0a2a1a', border: '#166534', color: '#6ee7b7', icon: '🟢' },
      good:      { bg: '#0a2a20', border: '#14532d', color: '#34d399', icon: '🟢' },
      moderate:  { bg: '#2a2a0a', border: '#854d0e', color: '#fbbf24', icon: '🟡' },
      poor:      { bg: '#2a0a0a', border: '#991b1b', color: '#f87171', icon: '🔴' },
    };
    return map[r] || map.moderate;
  }

  function severityColor(s) {
    if (s === 'high') return '#f87171';
    if (s === 'medium') return '#fbbf24';
    return '#34d399';
  }

  function trendIcon(t) {
    if (t === 'improving') return '📈';
    if (t === 'degrading') return '📉';
    return '➡️';
  }

  function tpsBarWidth(val, max) {
    return Math.min(100, Math.max(2, (val / max) * 100));
  }

  let maxTps = $derived(analysis ? Math.max(analysis.p95_tokens_per_second, analysis.avg_tokens_per_second, 1) * 1.2 : 50);

  onMount(() => { refresh(); });

  $effect(() => {
    if (autoRefresh) {
      const interval = setInterval(refresh, 3000);
      return () => clearInterval(interval);
    }
  });
</script>

<div class="perf-monitor">
  {#if loading}
    <div class="loading-state">Analyzing performance...</div>
  {:else if error}
    <div class="empty-state">
      <p>⚠️ {error}</p>
      <button onclick={refresh} class="btn-sm" style="margin-top:0.5rem">↻ Retry</button>
    </div>
  {:else if !analysis || !analysis.total_requests_analyzed}
    <div class="empty-state">
      <p>No performance data yet. Send some chat requests and come back to see analysis here.</p>
      {#if analysis?.suggestions?.length}
        <div class="suggestions-section" style="margin-top:1rem; text-align:left;">
          <h3>💡 Suggestions</h3>
          {#each analysis.suggestions as sug}
            <p style="color:#888; font-size:0.85rem; margin-top:0.3rem;">{sug.description}</p>
          {/each}
        </div>
      {/if}
    </div>
  {:else}
    {@const rs = ratingStyle(analysis.overall_rating)}

    <!-- Overall Rating Banner -->
    <div class="rating-banner" style="background:{rs.bg}; border-color:{rs.border};">
      <div class="rating-main">
        <span class="rating-icon">{rs.icon}</span>
        <div>
          <span class="rating-label" style="color:{rs.color}">{analysis.overall_rating.toUpperCase()}</span>
          <span class="rating-sublabel">overall performance</span>
        </div>
      </div>
      <div class="rating-meta">
        <span class="trend">{trendIcon(analysis.recent_trend)} {analysis.recent_trend || 'stable'}</span>
        {#if analysis.bottleneck && analysis.bottleneck !== 'none'}
          <span class="bottleneck">Bottleneck: <strong>{analysis.bottleneck}</strong></span>
        {/if}
      </div>
    </div>

    <!-- Speed Gauges -->
    <div class="gauges-grid">
      <div class="gauge-card">
        <span class="gauge-label">Avg Tokens/sec</span>
        <span class="gauge-value">{analysis.avg_tokens_per_second.toFixed(1)}</span>
        <div class="gauge-bar">
          <div class="gauge-fill" style="width:{tpsBarWidth(analysis.avg_tokens_per_second, maxTps)}%; background:#6ee7b7;"></div>
        </div>
      </div>
      <div class="gauge-card">
        <span class="gauge-label">P50 Tokens/sec</span>
        <span class="gauge-value">{analysis.p50_tokens_per_second.toFixed(1)}</span>
        <div class="gauge-bar">
          <div class="gauge-fill" style="width:{tpsBarWidth(analysis.p50_tokens_per_second, maxTps)}%; background:#34d399;"></div>
        </div>
      </div>
      <div class="gauge-card">
        <span class="gauge-label">P95 Tokens/sec</span>
        <span class="gauge-value">{analysis.p95_tokens_per_second.toFixed(1)}</span>
        <div class="gauge-bar">
          <div class="gauge-fill" style="width:{tpsBarWidth(analysis.p95_tokens_per_second, maxTps)}%; background:#8ecae6;"></div>
        </div>
      </div>
      <div class="gauge-card">
        <span class="gauge-label">Time to First Token</span>
        <span class="gauge-value">{analysis.avg_time_to_first_token_ms.toFixed(0)}<small>ms</small></span>
        <div class="gauge-bar">
          <div class="gauge-fill ttft" style="width:{Math.min(100, analysis.avg_time_to_first_token_ms / 20)}%; background:{analysis.avg_time_to_first_token_ms < 500 ? '#6ee7b7' : analysis.avg_time_to_first_token_ms < 1500 ? '#fbbf24' : '#f87171'};"></div>
        </div>
      </div>
    </div>

    <!-- Suggestions -->
    {#if analysis.suggestions && analysis.suggestions.length > 0}
      <div class="suggestions-section">
        <h3>💡 Optimization Suggestions</h3>
        <div class="suggestion-list">
          {#each analysis.suggestions as sug}
            <div class="suggestion-card">
              <div class="sug-header">
                <span class="sug-severity" style="color:{severityColor(sug.severity)}">{sug.severity.toUpperCase()}</span>
                <span class="sug-category">{sug.category}</span>
              </div>
              <p class="sug-title">{sug.title}</p>
              <p class="sug-desc">{sug.description}</p>
              {#if sug.config_change}
                <code class="sug-config">{sug.config_change}</code>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <div class="no-suggestions">
        <p>✅ No optimization issues detected — your current configuration looks good!</p>
      </div>
    {/if}

    <!-- Recent Samples Sparkline Table -->
    {#if samples.length > 0}
      <div class="samples-section">
        <h3>📊 Recent Inference Samples</h3>
        <div class="samples-table">
          <div class="sample-header">
            <span>Time</span><span>Model</span><span>Tok/s</span><span>Tokens</span><span>Duration</span><span>TTFT</span>
          </div>
          {#each samples.slice(-30).reverse() as s}
            <div class="sample-row">
              <span class="s-time">{new Date(s.timestamp).toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })}</span>
              <span class="s-model">{s.model || '-'}</span>
              <span class="s-tps" style="color:{s.tokens_per_second >= 20 ? '#6ee7b7' : s.tokens_per_second >= 10 ? '#fbbf24' : '#f87171'}">{s.tokens_per_second.toFixed(1)}</span>
              <span class="s-tok">{s.prompt_tokens}→{s.completion_tokens}</span>
              <span class="s-dur">{s.duration_ms}ms</span>
              <span class="s-ttft">{s.time_to_first_token_ms ? s.time_to_first_token_ms + 'ms' : '-'}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- AI Self-Optimization -->
    <div class="optimize-divider"></div>
    <AutoOptimize />

    <div class="refresh-bar">
      <label class="auto-toggle">
        <input type="checkbox" bind:checked={autoRefresh} />
        <span>Auto-refresh (3s)</span>
      </label>
      <button onclick={refresh} class="btn-sm">↻ Refresh Now</button>
    </div>
  {/if}
</div>

<style>
  .perf-monitor { display: flex; flex-direction: column; gap: 1rem; }

  .loading-state, .empty-state { text-align: center; padding: 2rem; color: #555; }

  .rating-banner {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.25rem;
    border: 1px solid;
    border-radius: 10px;
    flex-wrap: wrap;
    gap: 0.75rem;
  }
  .rating-main { display: flex; align-items: center; gap: 0.75rem; }
  .rating-icon { font-size: 1.8rem; }
  .rating-label { font-size: 1.3rem; font-weight: 700; display: block; }
  .rating-sublabel { font-size: 0.8rem; color: #888; }
  .rating-meta { display: flex; gap: 1.5rem; align-items: center; font-size: 0.85rem; color: #888; }
  .trend { text-transform: capitalize; }
  .bottleneck strong { color: #fbbf24; }

  .gauges-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0.75rem;
  }
  .gauge-card {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    padding: 0.75rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .gauge-label { font-size: 0.75rem; color: #888; text-transform: uppercase; letter-spacing: 0.03em; }
  .gauge-value { font-size: 1.6rem; font-weight: 700; color: #e0e0e8; line-height: 1.2; }
  .gauge-value small { font-size: 0.7rem; color: #888; font-weight: 400; }
  .gauge-bar { height: 4px; background: #1a1a2e; border-radius: 2px; overflow: hidden; }
  .gauge-fill { height: 100%; border-radius: 2px; transition: width 0.5s ease; }

  .suggestions-section h3, .samples-section h3 {
    font-size: 0.9rem; color: #e0e0e8; margin-bottom: 0.5rem;
  }
  .suggestion-list { display: flex; flex-direction: column; gap: 0.5rem; }
  .suggestion-card {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    padding: 0.75rem 1rem;
  }
  .sug-header { display: flex; gap: 0.75rem; align-items: center; margin-bottom: 0.3rem; }
  .sug-severity { font-size: 0.7rem; font-weight: 700; letter-spacing: 0.04em; }
  .sug-category { font-size: 0.72rem; color: #555; background: #1a1a2e; padding: 0.1rem 0.4rem; border-radius: 3px; }
  .sug-title { font-size: 0.88rem; color: #e0e0e8; font-weight: 600; margin-bottom: 0.2rem; }
  .sug-desc { font-size: 0.82rem; color: #888; line-height: 1.4; }
  .sug-config {
    display: block;
    margin-top: 0.4rem;
    background: #080810;
    border: 1px solid #2a2a40;
    border-radius: 4px;
    padding: 0.3rem 0.5rem;
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.78rem;
    color: #6ee7b7;
  }

  .no-suggestions {
    background: #0a2a1a;
    border: 1px solid #166534;
    border-radius: 8px;
    padding: 1rem;
    text-align: center;
    color: #6ee7b7;
    font-size: 0.9rem;
  }

  .samples-table {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    overflow: hidden;
  }
  .sample-header, .sample-row {
    display: grid;
    grid-template-columns: 70px 1fr 60px 80px 65px 60px;
    gap: 0.5rem;
    padding: 0.35rem 0.75rem;
    font-size: 0.8rem;
  }
  .sample-header {
    background: #0a0a14;
    color: #555;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-bottom: 1px solid #1e1e30;
  }
  .sample-row { border-bottom: 1px solid #111120; color: #aaa; }
  .s-time { font-family: 'JetBrains Mono', monospace; color: #666; font-size: 0.78rem; }
  .s-model { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .s-tps { font-weight: 600; }

  .refresh-bar {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 0.75rem;
    padding-top: 0.5rem;
  }
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
    background: #1a1a2e;
    color: #888;
  }
  .btn-sm:hover { background: #2a2a40; color: #ccc; }

  .optimize-divider {
    border-top: 1px solid #1e1e30;
    margin: 0.5rem 0;
  }

  @media (max-width: 700px) {
    .gauges-grid { grid-template-columns: 1fr 1fr; }
    .sample-header, .sample-row { grid-template-columns: 60px 1fr 55px 60px; }
    .sample-header span:nth-child(5), .sample-header span:nth-child(6),
    .sample-row span:nth-child(5), .sample-row span:nth-child(6) { display: none; }
  }
</style>
