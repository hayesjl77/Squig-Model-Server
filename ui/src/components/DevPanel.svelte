<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';

  let logs = $state([]);
  let loading = $state(true);
  let autoRefresh = $state(true);
  let modelFilter = $state('');
  let expandedEntry = $state(null);
  let showRequestBody = $state(true);
  let showResponseBody = $state(true);

  async function refreshLogs() {
    try {
      const params = new URLSearchParams({ limit: '200' });
      if (modelFilter) params.set('model', modelFilter);
      const res = await api.devLogs(200, modelFilter || undefined);
      logs = res.entries || [];
    } catch (_) {}
    loading = false;
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

  {#if loading}
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
</div>

<style>
  .dev-panel { display: flex; flex-direction: column; gap: 0.5rem; }

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
  }
</style>
