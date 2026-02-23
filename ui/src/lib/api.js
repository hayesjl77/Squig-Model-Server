const BASE = "";

/** @param {string} path */
async function get(path) {
  const res = await fetch(`${BASE}${path}`);
  if (!res.ok) throw new Error(`GET ${path}: ${res.status}`);
  return res.json();
}

/** @param {string} path @param {any} body */
async function post(path, body) {
  const res = await fetch(`${BASE}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`POST ${path}: ${res.status}`);
  return res.json();
}

export const api = {
  status: () => get("/api/status"),
  hardware: () => get("/api/hardware"),
  health: () => get("/api/health"),
  metrics: () => get("/api/metrics"),
  availableModels: () => get("/api/models/available"),
  loadedModels: () => get("/api/models/loaded"),
  loadModel: (/** @type {string} */ model) =>
    post("/api/models/load", { model }),
  unloadModel: (/** @type {string} */ model) =>
    post("/api/models/unload", { model }),
  models: () => get("/v1/models"),

  // HuggingFace integration
  hfSearch: (/** @type {string} */ q, /** @type {number} */ limit = 20) =>
    post("/api/hf/search", { q, limit }),
  hfDownload: (/** @type {string} */ repo_id, /** @type {string} */ filename) =>
    post("/api/hf/download", { repo_id, filename }),
  hfDownloadAndLoad: (
    /** @type {string} */ repo_id,
    /** @type {string} */ filename,
  ) => post("/api/hf/download-and-load", { repo_id, filename }),
  hfDownloads: () => get("/api/hf/downloads"),
  hfCancel: (/** @type {string} */ repo_id, /** @type {string} */ filename) =>
    post("/api/hf/cancel", { repo_id, filename }),
  hfClear: () => post("/api/hf/clear", {}),

  // Developer tools
  devLogs: (
    /** @type {number} */ limit = 200,
    /** @type {string|undefined} */ model,
  ) => {
    const params = new URLSearchParams({ limit: String(limit) });
    if (model) params.set("model", model);
    return get(`/api/dev/logs?${params}`);
  },
  devLogsClear: () => post("/api/dev/logs/clear", {}),
  devPerf: () => get("/api/dev/perf"),
  devPerfSamples: () => get("/api/dev/perf/samples"),

  // Self-optimization
  devSettings: () => get("/api/dev/settings"),
  devOptimize: () => post("/api/dev/optimize", {}),
  devApplySettings: (
    /** @type {Array<{setting: string, value: any}>} */ changes,
    /** @type {string|undefined} */ reloadModel,
    /** @type {boolean} */ saveToDisk = false,
  ) =>
    post("/api/dev/apply-settings", {
      changes,
      reload_model: reloadModel,
      save_to_disk: saveToDisk,
    }),
};
