/**
 * Shared reactive state (Svelte 5 runes; this file must keep the .svelte.js suffix).
 * Anything needed by more than one component lives here; per-view data (tasks,
 * clips, ...) is fetched by the view itself when it mounts.
 */
export const DEFAULT_ACCENT = 'rgb(157, 180, 255)';

export const ui = $state({
  expanded: false,
  tab: 'music', // music | clips | tasks | calendar | modes
  accent: DEFAULT_ACCENT, // follows album art
  toast: '', // short message shown at the bottom of the island
});

/**
 * Live progress state — shared across the whole shell.
 *   value   : 0-100 while progress is active; -1 = hidden
 *   label   : short description shown beside the ring (e.g. "Copying…")
 *   charging: true while a charger is connected
 *   battery : 0-1 battery level (or null = unavailable)
 */
export const progress = $state({
  value: -1,
  label: '',
  charging: false,
  battery: null,
});

let progressTimer;
export function setProgress(value, label = '') {
  progress.value = Math.max(0, Math.min(100, value));
  progress.label = label;
  clearTimeout(progressTimer);
  // auto-clear after 60 s of no updates (safety valve)
  if (value < 100) progressTimer = setTimeout(() => clearProgress(), 60_000);
  else progressTimer = setTimeout(() => clearProgress(), 1_800); // linger 1.8 s at 100%
}
export function clearProgress() {
  clearTimeout(progressTimer);
  progress.value = -1;
  progress.label = '';
}

export const media = $state({
  active: false, title: '', artist: '', album: '', playing: false,
  position_ms: 0, duration_ms: 0, sampled_at_ms: 0, art: null,
});

export function applyMedia(m) {
  Object.assign(media, m);
}

/** Playback position now, extrapolated from the last backend sample. */
export function livePosition() {
  const p = media.position_ms + (media.playing ? Date.now() - media.sampled_at_ms : 0);
  return media.duration_ms > 0 ? Math.min(p, media.duration_ms) : Math.max(p, 0);
}

let toastTimer;
export function toast(msg, ms = 1800) {
  ui.toast = String(msg);
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    ui.toast = '';
  }, ms);
}

/** Pick a vivid colour from the album art for the accent (12x12 downsample). */
export function accentFromArt(url) {
  return new Promise((resolve) => {
    if (!url) return resolve(DEFAULT_ACCENT);
    const img = new Image();
    img.onerror = () => resolve(DEFAULT_ACCENT);
    img.onload = () => {
      const c = document.createElement('canvas');
      c.width = c.height = 12;
      const x = c.getContext('2d', { willReadFrequently: true });
      x.drawImage(img, 0, 0, 12, 12);
      const d = x.getImageData(0, 0, 12, 12).data;
      let best = null, top = -1;
      for (let i = 0; i < d.length; i += 4) {
        const mx = Math.max(d[i], d[i + 1], d[i + 2]);
        const score = (mx - Math.min(d[i], d[i + 1], d[i + 2])) * (0.4 + mx / 255);
        if (mx > 70 && score > top) { top = score; best = [d[i], d[i + 1], d[i + 2]]; }
      }
      if (!best) return resolve(DEFAULT_ACCENT);
      const [r, g, b] = best.map((v) => Math.round(v + (255 - v) * 0.35)); // lift for dark UI
      resolve(`rgb(${r}, ${g}, ${b})`);
    };
    img.src = url;
  });
}
