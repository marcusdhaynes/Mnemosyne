/**
 * Bridge from the native catalog to Spin's resolveMedia(..., {indexed}).
 *
 * `indexed` is an object keyed by file path:
 *   { [path]: { takenAt?: number, gps?: { lat, lon } } }
 *
 * Native/EXIF values are the only ones included, and they win over sidecars.
 * This module does not pair Live Photos, group bursts, extract Motion Photos,
 * or parse export sidecars — those belong to web/formats/.
 */

const SLASH = /\\/g;

/** Add a forward-slash alias so browser/export paths can match Windows paths. */
export function normalizeIndexed(raw) {
  const out = {};
  if (!raw || typeof raw !== "object") return out;
  for (const [path, value] of Object.entries(raw)) {
    if (!path || typeof value !== "object" || value === null) continue;
    const entry = shapeEntry(value);
    if (!entry) continue;
    out[path] = entry;
    const posix = path.replace(SLASH, "/");
    if (posix !== path && out[posix] === undefined) out[posix] = entry;
  }
  return out;
}

function shapeEntry(value) {
  const takenAt = toMs(value.takenAt ?? value.taken_at);
  const gpsIn = value.gps;
  let gps;
  if (gpsIn && Number.isFinite(Number(gpsIn.lat)) && Number.isFinite(Number(gpsIn.lon))) {
    gps = { lat: Number(gpsIn.lat), lon: Number(gpsIn.lon) };
  }
  if (takenAt == null && !gps) return null;
  const entry = {};
  if (takenAt != null) entry.takenAt = takenAt;
  if (gps) entry.gps = gps;
  return entry;
}

function toMs(v) {
  if (v == null || v === "") return null;
  const n = typeof v === "number" ? v : Number(v);
  return Number.isFinite(n) ? n : null;
}

/**
 * Load camera dates and GPS from the native catalog.
 * `invoke` is `window.__TAURI__.core.invoke` (or a test stub).
 * Returns a plain object suitable for `resolveMedia(files, {indexed})`.
 */
export async function loadIndexed(folderPath, invoke) {
  if (typeof invoke !== "function") return {};
  const raw = await invoke("catalog_indexed", { path: folderPath });
  return normalizeIndexed(raw);
}

/** Merge a native `indexed` object into a resolveMedia options bag. */
export function withIndexed(options, indexed) {
  return { ...options, indexed: normalizeIndexed(indexed) };
}
