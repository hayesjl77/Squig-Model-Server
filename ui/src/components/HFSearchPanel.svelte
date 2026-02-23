<script>
  import { api } from '../lib/api.js';

  let searchQuery = $state('');
  let searchResults = $state([]);
  let searching = $state(false);
  let searchError = $state('');
  let downloads = $state([]);
  let expandedRepo = $state(null);
  let autoLoad = $state(true);
  let hardware = $state(null);

  // Compute the best available memory for inference (VRAM or system RAM)
  function getAvailableMemoryMb() {
    if (!hardware) return null;
    // Use dedicated GPU VRAM if detected
    const gpuVram = hardware.gpus
      ?.map(g => g.vram_mb)
      .filter(v => v != null);
    if (gpuVram && gpuVram.length > 0) {
      // Sum all GPU VRAM (multi-GPU) or just take max for single
      return Math.max(...gpuVram);
    }
    // For integrated/unified memory (AMD APUs, etc.), use available system RAM
    // Also detect if this is likely a unified memory system (AMD with no discrete VRAM reported)
    if (hardware.available_memory_gb) {
      return hardware.available_memory_gb * 1024; // convert GB -> MB
    }
    if (hardware.total_memory_gb) {
      return hardware.total_memory_gb * 1024 * 0.8; // assume ~80% usable
    }
    return null;
  }

  function getTotalSystemMemoryMb() {
    if (!hardware) return null;
    return (hardware.total_memory_gb || 0) * 1024;
  }

  /**
   * Rate a model file against detected hardware.
   * Returns { level, label, tooltip }
   *   level: 'good' | 'tight' | 'warning' | 'impossible' | 'unknown'
   */
  function rateModel(fileSizeBytes) {
    if (!hardware || !fileSizeBytes) return { level: 'unknown', label: '', tooltip: '' };

    const fileSizeMb = fileSizeBytes / (1024 * 1024);
    const overheadMb = 2048; // ~2 GB for KV cache, context, OS overhead
    const requiredMb = fileSizeMb + overheadMb;

    const vramMb = getAvailableMemoryMb();
    const totalSysMb = getTotalSystemMemoryMb();

    if (vramMb == null && totalSysMb == null) {
      return { level: 'unknown', label: '', tooltip: '' };
    }

    const bestAvailable = vramMb || totalSysMb;
    const fileSizeGb = (fileSizeMb / 1024).toFixed(1);
    const availGb = (bestAvailable / 1024).toFixed(1);

    // Check against total system RAM too (absolute ceiling)
    if (totalSysMb && requiredMb > totalSysMb) {
      return {
        level: 'impossible',
        label: '🚫 Cannot run',
        tooltip: `Model needs ~${fileSizeGb} GB + ${(overheadMb/1024).toFixed(0)} GB overhead = ~${((fileSizeMb + overheadMb)/1024).toFixed(1)} GB. Your system has ${(totalSysMb/1024).toFixed(1)} GB total RAM. This model cannot fit in memory.`,
      };
    }

    const ratio = requiredMb / bestAvailable;

    if (ratio > 1.0) {
      return {
        level: 'impossible',
        label: '🚫 Cannot run',
        tooltip: `Model needs ~${fileSizeGb} GB + overhead but you only have ${availGb} GB available. This will not fit in memory.`,
      };
    }
    if (ratio > 0.9) {
      return {
        level: 'warning',
        label: '⚠️ Very tight',
        tooltip: `Model needs ~${fileSizeGb} GB + overhead — uses >90% of your ${availGb} GB. Expect heavy swapping and very slow inference.`,
      };
    }
    if (ratio > 0.7) {
      return {
        level: 'tight',
        label: '⚡ Tight fit',
        tooltip: `Model needs ~${fileSizeGb} GB + overhead — uses ~${Math.round(ratio * 100)}% of your ${availGb} GB. Should work but may be slow with large contexts.`,
      };
    }
    return {
      level: 'good',
      label: '✅ Good fit',
      tooltip: `Model needs ~${fileSizeGb} GB + overhead — fits comfortably in your ${availGb} GB.`,
    };
  }

  async function search() {
    if (!searchQuery.trim()) return;
    searching = true;
    searchError = '';
    searchResults = [];
    try {
      const res = await api.hfSearch(searchQuery.trim());
      if (res.error) {
        searchError = res.error;
      } else {
        searchResults = res.results || [];
      }
    } catch (e) {
      searchError = e.message;
    } finally {
      searching = false;
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Enter') search();
  }

  async function downloadFile(repoId, filename) {
    try {
      if (autoLoad) {
        await api.hfDownloadAndLoad(repoId, filename);
      } else {
        await api.hfDownload(repoId, filename);
      }
      refreshDownloads();
    } catch (e) {
      searchError = `Download failed: ${e.message}`;
    }
  }

  async function cancelDownload(repoId, filename) {
    try {
      await api.hfCancel(repoId, filename);
    } catch (e) {
      searchError = `Cancel failed: ${e.message}`;
    }
  }

  async function refreshDownloads() {
    try {
      const res = await api.hfDownloads();
      downloads = res.downloads || [];
    } catch (_) {}
  }

  async function clearFinished() {
    await api.hfClear();
    refreshDownloads();
  }

  function toggleRepo(repoId) {
    expandedRepo = expandedRepo === repoId ? null : repoId;
  }

  function formatNumber(n) {
    if (!n) return '0';
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
    if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K';
    return n.toString();
  }

  function isDownloading(repoId, filename) {
    return downloads.some(
      d => d.repo_id === repoId && d.filename === filename &&
           (d.status === 'downloading' || d.status === 'queued')
    );
  }

  function getDownload(repoId, filename) {
    return downloads.find(d => d.repo_id === repoId && d.filename === filename);
  }

  /** Get the best (most runnable) rating across all GGUF files in a repo */
  function getBestRepoRating(ggufFiles) {
    if (!hardware || !ggufFiles?.length) return null;
    const levels = ['good', 'tight', 'warning', 'impossible', 'unknown'];
    let best = null;
    for (const file of ggufFiles) {
      const r = rateModel(file.size);
      if (!best || levels.indexOf(r.level) < levels.indexOf(best.level)) {
        best = r;
      }
    }
    return best;
  }

  // Poll downloads every 2 seconds if any are active
  $effect(() => {
    const hasActive = downloads.some(d => d.status === 'downloading' || d.status === 'queued');
    if (hasActive) {
      const interval = setInterval(refreshDownloads, 2000);
      return () => clearInterval(interval);
    }
  });

  // Initial fetch of existing downloads + hardware
  import { onMount } from 'svelte';
  onMount(async () => {
    refreshDownloads();
    try {
      hardware = await api.hardware();
    } catch (_) {}
  });
</script>

<div class="hf-panel">
  <!-- Search Section -->
  <div class="search-section">
    <div class="search-bar">
      <input
        type="text"
        bind:value={searchQuery}
        onkeydown={handleKeydown}
        placeholder="Search HuggingFace for GGUF models... (e.g. Qwen 32B, Llama 3, DeepSeek)"
        class="search-input"
      />
      <button onclick={search} disabled={searching || !searchQuery.trim()} class="btn-search">
        {searching ? 'Searching...' : 'Search'}
      </button>
    </div>
    <div class="search-options">
      <label class="auto-load-toggle">
        <input type="checkbox" bind:checked={autoLoad} />
        <span>Auto-load after download</span>
      </label>
    </div>
  </div>

  {#if searchError}
    <div class="error-msg">{searchError}</div>
  {/if}

  <!-- Active Downloads -->
  {#if downloads.length > 0}
    <div class="downloads-section">
      <div class="section-header">
        <h3>Downloads</h3>
        <button onclick={clearFinished} class="btn-sm btn-clear">Clear Finished</button>
      </div>
      {#each downloads as dl}
        <div class="download-item" class:complete={dl.status === 'complete'} class:failed={dl.status === 'failed'}>
          <div class="dl-info">
            <span class="dl-filename">{dl.filename}</span>
            <span class="dl-repo">{dl.repo_id}</span>
          </div>
          <div class="dl-progress">
            {#if dl.status === 'downloading'}
              <div class="progress-bar">
                <div class="progress-fill" style="width: {dl.progress_pct}%"></div>
              </div>
              <span class="dl-pct">{dl.progress_pct}%</span>
              <button onclick={() => cancelDownload(dl.repo_id, dl.filename)} class="btn-sm btn-cancel">Cancel</button>
            {:else if dl.status === 'queued'}
              <span class="dl-status queued">Queued</span>
            {:else if dl.status === 'complete'}
              <span class="dl-status complete">✓ Complete</span>
            {:else if dl.status === 'failed'}
              <span class="dl-status failed">✗ {dl.error || 'Failed'}</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Search Results -->
  {#if searching}
    <div class="loading-results">
      <div class="spinner"></div>
      <p>Searching HuggingFace...</p>
    </div>
  {:else if searchResults.length > 0}
    <div class="results-section">
      <h3>Results ({searchResults.length})</h3>
      {#each searchResults as result}
        {@const bestRating = getBestRepoRating(result.gguf_files)}
        <div class="repo-card">
          <button class="repo-header" onclick={() => toggleRepo(result.repo_id)}>
            <div class="repo-info">
              <span class="repo-name">{result.repo_id}</span>
              <div class="repo-meta">
                <span class="meta-item">⬇ {formatNumber(result.downloads)}</span>
                <span class="meta-item">♥ {formatNumber(result.likes)}</span>
                <span class="meta-item">{result.gguf_files.length} GGUF file{result.gguf_files.length !== 1 ? 's' : ''}</span>
                {#if bestRating && bestRating.level !== 'unknown'}
                  <span class="meta-item hw-badge hw-{bestRating.level}" title={bestRating.tooltip}>{bestRating.label}</span>
                {/if}
              </div>
            </div>
            <span class="expand-icon">{expandedRepo === result.repo_id ? '▼' : '▶'}</span>
          </button>

          {#if expandedRepo === result.repo_id}
            <div class="gguf-files">
              {#if hardware}
                <div class="hw-summary">
                  <span class="hw-summary-label">Your hardware:</span>
                  <span class="hw-summary-value">
                    {hardware.gpus?.length ? hardware.gpus.map(g => `${g.name}${g.vram_mb ? ` (${(g.vram_mb/1024).toFixed(0)} GB)` : ''}`).join(', ') : 'No GPU detected'}
                    · {hardware.total_memory_gb?.toFixed(0) || '?'} GB RAM ({hardware.available_memory_gb?.toFixed(0) || '?'} GB free)
                  </span>
                </div>
              {/if}
              <table>
                <thead>
                  <tr>
                    <th>Filename</th>
                    <th>Size</th>
                    <th>Fit</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {#each result.gguf_files as file}
                    {@const dl = getDownload(result.repo_id, file.filename)}
                    {@const rating = rateModel(file.size)}
                    <tr class="file-row" class:row-impossible={rating.level === 'impossible'}>
                      <td class="file-name">{file.filename}</td>
                      <td class="file-size">{file.size_human}</td>
                      <td class="file-fit">
                        {#if rating.level !== 'unknown'}
                          <span class="fit-badge fit-{rating.level}" title={rating.tooltip}>
                            {rating.label}
                          </span>
                        {/if}
                      </td>
                      <td class="file-action">
                        {#if dl && (dl.status === 'downloading' || dl.status === 'queued')}
                          <div class="inline-progress">
                            <div class="progress-bar small">
                              <div class="progress-fill" style="width: {dl.progress_pct}%"></div>
                            </div>
                            <span class="dl-pct-sm">{dl.progress_pct}%</span>
                          </div>
                        {:else if dl && dl.status === 'complete'}
                          <span class="dl-done">✓ Done</span>
                        {:else}
                          <button
                            onclick={() => downloadFile(result.repo_id, file.filename)}
                            class="btn-sm btn-download"
                            class:btn-download-warn={rating.level === 'warning' || rating.level === 'impossible'}
                            title={rating.level === 'impossible' ? 'This model will not fit in your available memory' : ''}
                          >
                            {#if rating.level === 'impossible'}
                              Download Anyway
                            {:else}
                              Download{autoLoad ? ' & Load' : ''}
                            {/if}
                          </button>
                        {/if}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {:else if searchQuery && !searching && searchResults.length === 0 && !searchError}
    <div class="no-results">
      <p>No GGUF models found for "{searchQuery}". Try a different search term.</p>
    </div>
  {/if}
</div>

<style>
  .hf-panel { display: flex; flex-direction: column; gap: 1rem; }

  .search-section {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 10px;
    padding: 1.25rem;
  }

  .search-bar {
    display: flex;
    gap: 0.5rem;
  }

  .search-input {
    flex: 1;
    padding: 0.6rem 1rem;
    border: 1px solid #2a2a40;
    border-radius: 6px;
    background: #080810;
    color: #e0e0e8;
    font-size: 0.95rem;
    outline: none;
    transition: border-color 0.15s;
  }
  .search-input:focus { border-color: #6ee7b7; }
  .search-input::placeholder { color: #555; }

  .btn-search {
    padding: 0.6rem 1.5rem;
    border: none;
    border-radius: 6px;
    background: #1a3328;
    color: #6ee7b7;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.15s;
  }
  .btn-search:hover:not(:disabled) { background: #244a38; }
  .btn-search:disabled { opacity: 0.5; cursor: not-allowed; }

  .search-options {
    margin-top: 0.75rem;
    display: flex;
    gap: 1rem;
  }

  .auto-load-toggle {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    color: #888;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .auto-load-toggle input { accent-color: #6ee7b7; }

  .error-msg {
    background: #331a1a;
    color: #f87171;
    padding: 0.75rem 1rem;
    border-radius: 6px;
    font-size: 0.9rem;
  }

  /* Downloads */
  .downloads-section {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 10px;
    padding: 1.25rem;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.75rem;
  }
  .section-header h3 {
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #6ee7b7;
  }

  .download-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.6rem 0;
    border-bottom: 1px solid #151525;
    gap: 1rem;
  }
  .download-item:last-child { border-bottom: none; }

  .dl-info { flex: 1; min-width: 0; }
  .dl-filename { font-weight: 500; font-size: 0.9rem; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dl-repo { font-size: 0.75rem; color: #666; }

  .dl-progress { display: flex; align-items: center; gap: 0.5rem; flex-shrink: 0; }

  .progress-bar {
    width: 120px;
    height: 6px;
    background: #1a1a2e;
    border-radius: 3px;
    overflow: hidden;
  }
  .progress-bar.small { width: 80px; height: 4px; }
  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #6ee7b7, #34d399);
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .dl-pct { font-size: 0.8rem; color: #6ee7b7; min-width: 38px; text-align: right; }
  .dl-pct-sm { font-size: 0.75rem; color: #6ee7b7; }

  .dl-status { font-size: 0.8rem; font-weight: 500; }
  .dl-status.queued { color: #fbbf24; }
  .dl-status.complete { color: #6ee7b7; }
  .dl-status.failed { color: #f87171; }

  .inline-progress { display: flex; align-items: center; gap: 0.4rem; }

  .dl-done { color: #6ee7b7; font-size: 0.8rem; font-weight: 500; }

  /* Results */
  .loading-results {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 2rem;
    gap: 0.75rem;
    color: #666;
  }
  .spinner {
    width: 24px; height: 24px;
    border: 3px solid #222;
    border-top-color: #6ee7b7;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .results-section h3 {
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #6ee7b7;
    margin-bottom: 0.75rem;
  }

  .repo-card {
    background: #0e0e1a;
    border: 1px solid #1e1e30;
    border-radius: 8px;
    margin-bottom: 0.5rem;
    overflow: hidden;
  }

  .repo-header {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    text-align: left;
    transition: background 0.15s;
  }
  .repo-header:hover { background: #12121e; }

  .repo-info { flex: 1; min-width: 0; }
  .repo-name {
    font-weight: 600;
    font-size: 0.95rem;
    color: #e0e0e8;
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .repo-meta {
    display: flex;
    gap: 0.75rem;
    margin-top: 0.25rem;
  }
  .meta-item { font-size: 0.8rem; color: #666; }

  .expand-icon { color: #555; font-size: 0.7rem; flex-shrink: 0; margin-left: 0.5rem; }

  .gguf-files {
    border-top: 1px solid #1e1e30;
    padding: 0.5rem;
  }

  table { width: 100%; border-collapse: collapse; }
  th {
    text-align: left;
    padding: 0.4rem 0.6rem;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #555;
    border-bottom: 1px solid #1a1a2e;
  }
  td {
    padding: 0.4rem 0.6rem;
    font-size: 0.85rem;
    border-bottom: 1px solid #0e0e1a;
  }
  .file-name {
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
    color: #c4c4d0;
    word-break: break-all;
  }
  .file-size { color: #888; white-space: nowrap; }
  .file-action { text-align: right; white-space: nowrap; }

  .btn-sm {
    padding: 0.25rem 0.6rem;
    border-radius: 4px;
    border: none;
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 500;
  }
  .btn-download { background: #1a3328; color: #6ee7b7; }
  .btn-download:hover { background: #244a38; }
  .btn-cancel { background: #4a1a1a; color: #f87171; }
  .btn-cancel:hover { background: #5a2020; }
  .btn-clear { background: #1a1a2e; color: #888; font-size: 0.75rem; }
  .btn-clear:hover { background: #2a2a40; color: #ccc; }

  .no-results {
    text-align: center;
    padding: 2rem;
    color: #555;
  }

  /* Hardware recommendation badges */
  .hw-badge {
    font-weight: 500;
    font-size: 0.75rem;
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
  }
  .hw-good { color: #6ee7b7; background: #1a3328; }
  .hw-tight { color: #fbbf24; background: #332d1a; }
  .hw-warning { color: #fb923c; background: #33261a; }
  .hw-impossible { color: #f87171; background: #331a1a; }

  .hw-summary {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.6rem;
    margin-bottom: 0.5rem;
    background: #080810;
    border-radius: 4px;
    font-size: 0.8rem;
  }
  .hw-summary-label { color: #555; white-space: nowrap; }
  .hw-summary-value { color: #888; }

  .file-fit { white-space: nowrap; }

  .fit-badge {
    font-size: 0.75rem;
    font-weight: 500;
    padding: 0.15rem 0.4rem;
    border-radius: 3px;
    cursor: help;
  }
  .fit-good { color: #6ee7b7; background: #1a3328; }
  .fit-tight { color: #fbbf24; background: #332d1a; }
  .fit-warning { color: #fb923c; background: #33261a; }
  .fit-impossible { color: #f87171; background: #331a1a; }

  .row-impossible { opacity: 0.6; }
  .row-impossible:hover { opacity: 0.85; }

  .btn-download-warn {
    background: #33261a !important;
    color: #fb923c !important;
  }
  .btn-download-warn:hover {
    background: #4a361a !important;
  }
</style>
