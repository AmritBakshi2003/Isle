/**
 * The ONLY place the UI talks to the backend.
 *   call(cmd, args)  -> invoke a Rust command (src-tauri/src/commands.rs etc.)
 *   on(event, cb)    -> subscribe to a backend event; returns an unsubscribe fn
 *
 * Outside Tauri (plain `npm run dev` in a browser) every call is served by the
 * in-memory mock below, so the whole UI can be developed and screenshot-tested
 * without Rust or Windows. Keep the mock's behaviour in step with the Rust side.
 * Args are camelCase here (Tauri converts to snake_case in Rust).
 */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export function call(cmd, args = {}) {
  return inTauri ? invoke(cmd, args) : Promise.resolve().then(() => mock(cmd, args));
}

const bus = new Map();
export function on(event, cb) {
  if (inTauri) {
    const p = listen(event, (e) => cb(e.payload));
    return () => p.then((un) => un());
  }
  if (!bus.has(event)) bus.set(event, new Set());
  bus.get(event).add(cb);
  return () => bus.get(event)?.delete(cb);
}
const emit = (event, payload) => bus.get(event)?.forEach((cb) => cb(payload));

/* ------------------------------------------------------------ browser mock */
const sec = (d = new Date()) => Math.floor(d.getTime() / 1000);
const at = (h, m = 0, plusDays = 0) => {
  const d = new Date();
  d.setDate(d.getDate() + plusDays);
  d.setHours(h, m, 0, 0);
  return sec(d);
};
const parseDateTime = (dateStr, timeStr) => {
  if (!dateStr) return null;
  const [year, month, day] = dateStr.split('-').map(Number);
  const date = new Date(year, month - 1, day);
  if (timeStr) {
    const [hours, minutes] = timeStr.split(':').map(Number);
    date.setHours(hours, minutes, 0, 0);
  }
  return sec(date);
};
const art = (c1, c2) =>
  'data:image/svg+xml;utf8,' +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${c1}"/><stop offset="1" stop-color="${c2}"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/><circle cx="50" cy="50" r="22" fill="none" stroke="#fff" stroke-opacity=".5" stroke-width="3"/></svg>`,
  );

const M = {
  active: true, title: 'Night Drive', artist: 'Lowtide', album: 'Harbor Lights',
  playing: true, position_ms: 61000, duration_ms: 214000, sampled_at_ms: Date.now(),
  art: art('#ff7a59', '#5b2a86'),
};
let seq = 100;

/* Workspace dashboard mock state. The sampler only ticks while the Workspace
   tab has called net_sampler_set(true) — same duty cycle as the Rust one. */
const net = { on: false, timer: null, seq: 0, down: 0 };
const netSample = () => {
  // a burst every handful of seconds so the sparkline has shape
  const burst = ++net.seq % 7 === 0;
  const target = burst ? 4e6 + Math.random() * 1.2e7 : 40e3 + Math.random() * 500e3;
  net.down = Math.round(net.down * 0.55 + target * 0.45);
  return {
    down_bps: net.down,
    up_bps: Math.round(net.down * (0.04 + Math.random() * 0.14)),
    link_down_bps: 1_000_000_000,
    link_up_bps: 1_000_000_000,
    iface: 'Wi-Fi',
    latency_ms: Math.random() < 0.03 ? null : 9 + Math.round(Math.random() * 14),
    ts: sec(),
  };
};
let lastNet = null;
const netGate = (on) => {
  net.on = on;
  clearInterval(net.timer);
  net.timer = null;
  if (!on) return;
  net.timer = setInterval(() => {
    lastNet = netSample();
    emit('workspace://network', lastNet);
  }, 1000);
};

/* Resource monitor mock: five believable apps whose load drifts each tick. */
const MOCK_APPS = [
  { pid: 4321, hwnd: 131082, app: 'Code.exe', title: 'isle: src/App.svelte', cpu: 14.2, mem: 3.1, gpu: 0.8 },
  { pid: 5122, hwnd: 262280, app: 'msedgewebview2.exe', title: 'Tauri 2 — WebView2', cpu: 7.5, mem: 4.6, gpu: 3.4 },
  { pid: 6033, hwnd: 393356, app: 'Teams.exe', title: 'Design sync | Microsoft Teams', cpu: 3.1, mem: 2.8, gpu: 1.9 },
  { pid: 7144, hwnd: 524432, app: 'chrome.exe', title: 'Tauri 2 docs', cpu: 1.4, mem: 2.2, gpu: 0.4 },
  { pid: 8255, hwnd: 655508, app: 'explorer.exe', title: 'Downloads', cpu: 0.3, mem: 0.9, gpu: 0.0 },
];
const drift = (v) => Math.max(0, Math.min(100, v + (Math.random() - 0.55) * Math.max(0.3, v * 0.35)));
const apps = { on: false, timer: null, rows: [], keep: new Set() };
const appsSample = () => {
  apps.rows = MOCK_APPS.map((a) => ({
    hwnd: a.hwnd,
    pid: a.pid,
    app: a.app,
    title: a.title,
    cpu_pct: (a.cpu = drift(a.cpu)),
    mem_pct: (a.mem = drift(a.mem) * 0.4 + a.mem * 0.6),
    gpu_pct: (a.gpu = drift(a.gpu)),
    topmost: apps.keep.has(a.hwnd),
  })).sort((x, y) => (y.cpu_pct + y.mem_pct + y.gpu_pct) - (x.cpu_pct + x.mem_pct + x.gpu_pct));
  return {
    rows: apps.rows,
    gpu_available: true,
    cores: navigator.hardwareConcurrency || 4,
    mem_total_mb: 16264,
    ts: sec(),
  };
};
let lastApps = null;
const appsGate = (on) => {
  apps.on = on;
  clearInterval(apps.timer);
  apps.timer = null;
  if (!on) return;
  apps.timer = setInterval(() => {
    lastApps = appsSample();
    emit("workspace://apps", lastApps);
  }, 1000);
};

/* Digital wellbeing mock: a believable working day, most recent first. */
const wbRow = (app, minutes, title) => ({
  app, title, seconds: Math.round(minutes * 60), started: sec() - Math.round(minutes * 60), ended: sec(),
});
const wellbeingRows = () => [
  wbRow('Code.exe', 96, 'isle: src/App.svelte'),
  wbRow('chrome.exe', 41, 'Tauri 2 docs'),
  wbRow('Teams.exe', 24, 'Design sync'),
  wbRow('Alteryx.exe', 18, 'ETL workflow'),
  wbRow('explorer.exe', 9, 'Downloads'),
];
const WINDOWS = () => [
  { hwnd: 131082, pid: 4321, title: 'isle: src/App.svelte — Visual Studio Code', process: 'Code.exe' },
  { hwnd: 262280, pid: 5122, title: 'Tauri 2 — WebView2', process: 'msedgewebview2.exe' },
  { hwnd: 393356, pid: 6033, title: 'Design sync | Microsoft Teams', process: 'Teams.exe' },
  { hwnd: 524432, pid: 8255, title: 'File Explorer', process: 'explorer.exe' },
];
const D = {
  settings: {
    pill_size: 'medium', pill_position: 'center', corner_radius: '22',
    bg_color: '#1a1a1a', bg_blur: 'auto', opacity: '95',
    border_on: 'false', border_color: '#6c6c6c',
    // behavior
    auto_hide: 'false', auto_hide_time: '30',
    hide_fullscreen: 'true', hide_battery_saver: 'false',
    click_action: 'expand', always_on_top: 'true',
    blur_on_battery: 'false',
    // modules
    module_media: 'true', module_calendar: 'true', module_tasks: 'true',
    module_clipboard: 'true', module_workspace: 'true',
    // weather: off until a place is chosen (same as the Rust backend)
    weather_enabled: 'false', weather_place: '', weather_units: 'c',
    // media
    media_album_art: 'true', media_progress_ring: 'true',
    media_update_speed: '1000',
    // progress
    progress_file_copy: 'true', progress_download: 'true',
    progress_app_install: 'true', progress_battery_stretch: 'true',
    progress_ring_style: 'arc', progress_auto_dismiss: '3',
    // behavior
    blur_on_battery: 'true',
    // hotkeys
    hotkey_toggle: 'ctrl+alt+space', hotkey_settings: 'ctrl+alt+comma',
    // advanced
    log_level: 'error',
    // workspace: local-only defaults (latency probe stays on the LAN)
    wellbeing_enabled: 'true', net_ping_public: '0', net_ping_host: '1.1.1.1',
  },
  tasks: [
    { id: 1, title: 'Send the design review notes', done: false, due: null, remind_min: 0, day: null, done_at: null, notified: false },
    { id: 2, title: 'Renew the domain', done: false, due: null, remind_min: 0, day: null, done_at: null, notified: false },
    { id: 3, title: 'Book dentist', done: true, due: null, remind_min: 0, day: null, done_at: null, notified: false },
  ],
  events: [
    { id: 1, title: 'Standup', start: at(10, 0), end: null, remind_min: 10, color: null, notified: false },
    { id: 2, title: 'Design review', start: at(15, 30), end: null, remind_min: 10, color: null, notified: false },
    { id: 3, title: 'Lunch with Asha', start: at(13, 0, 2), end: null, remind_min: 30, color: null, notified: false },
  ],
  clips: [
    { id: 1, text: 'https://tauri.app/start/', pinned: true, ts: sec() - 4000, file_path: null, file_name: null, file_type: null },
    { id: 2, text: 'SELECT * FROM events WHERE starts > strftime(\'%s\',\'now\');', pinned: false, ts: sec() - 600, file_path: null, file_name: null, file_type: null },
    { id: 3, text: 'Meeting moved to 3:30, same room.', pinned: false, ts: sec() - 90, file_path: null, file_name: null, file_type: null },
    { id: 4, text: 'document.pdf', pinned: false, ts: sec() - 10, file_path: '/tmp/document.pdf', file_name: 'document.pdf', file_type: 'pdf' },
  ],
};
const del = (k, id) => (D[k] = D[k].filter((x) => x.id !== id));

/* Weather mock: no network in the browser. A place lookup produces a stable
   snapshot derived from the name, so the chip is exercised end to end. */
const W = { lat: null, lon: null, cache: null };
function mockWeather() {
  const units = D.settings.weather_units === 'f' ? 'f' : 'c';
  if (D.settings.weather_enabled !== 'true' || W.lat === null) {
    return { ok: false, place: D.settings.weather_place || '', units, error: 'Set a location in Settings' };
  }
  if (!W.cache) {
    // deterministic pseudo-conditions from the coordinates
    const seed = Math.abs(Math.round(W.lat * 100) + Math.round(W.lon * 100));
    const code = [0, 1, 2, 3, 61, 80][seed % 6];
    W.cache = {
      temp_c: 18 + (seed % 14), feels_c: 19 + (seed % 13),
      humidity: 40 + (seed % 45), wind_kph: 4 + (seed % 18), code,
    };
  }
  return { ok: true, place: D.settings.weather_place, units, fetched_at: sec(),
    temp_c: W.cache.temp_c, feels_c: W.cache.feels_c, humidity: W.cache.humidity,
    wind_kph: W.cache.wind_kph, code: W.cache.code, error: null };
}
const setMockPlace = (query) => {
  const name = String(query || '').trim();
  if (!name) throw new Error('Type a city or town');
  D.settings.weather_place = name.replace(/w/g, (c) => c.toUpperCase());
  D.settings.weather_enabled = 'true';
  W.lat = 19.07; W.lon = 72.88; W.cache = null;
  const w = mockWeather();
  emit('weather://update', w);
  return w;
};

function mock(cmd, a) {
  switch (cmd) {
    case 'island_resize': return;
    case 'media_get': return { ...M, sampled_at_ms: Date.now() };
    case 'media_control':
      if (a.cmd === 'play_pause') { M.position_ms += Date.now() - M.sampled_at_ms; M.playing = !M.playing; }
      else {
        const next = a.cmd === 'next';
        M.title = next ? 'Glass Hours' : 'Night Drive';
        M.artist = next ? 'Marble Sky' : 'Lowtide';
        M.position_ms = 0;
        M.art = next ? art('#4cc9f0', '#3a0ca3') : art('#ff7a59', '#5b2a86');
      }
      M.sampled_at_ms = Date.now();
      emit('media://update', { ...M });
      return;
    case 'task_list': return [...D.tasks].sort((x, y) => x.done - y.done || y.id - x.id).map(t => ({ id: t.id, title: t.title, done: t.done }));
    case 'task_add': {
      const title = a.title?.trim();
      if (!title) throw new Error('Task title cannot be empty');
      D.tasks.push({ id: ++seq, title, done: false, due: null, remind_min: 0, day: null, done_at: null, notified: false });
      return seq;
    }
    case 'task_toggle': {
      const t = D.tasks.find((x) => x.id === a.id);
      if (t) t.done = !t.done;
      return;
    }
    case 'task_delete': return del('tasks', a.id);
    case 'task_clear_done': {
      const beforeLen = D.tasks.length;
      D.tasks = D.tasks.filter((t) => !t.done);
      return beforeLen - D.tasks.length;
    }
    case 'events_for_day': {
      const tzOffsetMin = a.tzOffsetMin ?? -new Date().getTimezoneOffset();
      // Parse the date string and get start/end of that day
      const [year, month, day] = a.date.split('-').map(Number);
      const dayStart = new Date(year, month - 1, day, 0, 0, 0);
      const dayEnd = new Date(year, month - 1, day, 23, 59, 59);
      const startTs = Math.floor(dayStart.getTime() / 1000);
      const endTs = Math.floor(dayEnd.getTime() / 1000);
      return D.events.filter((e) => e.start >= startTs && e.start <= endTs);
    }
    case 'month_event_days': {
      const tzOffsetMin = a.tzOffsetMin ?? -new Date().getTimezoneOffset();
      // Return dates that have events (mock data for current month)
      const now = new Date();
      const year = a.year || now.getFullYear();
      const month = a.month || now.getMonth() + 1;
      // For mock, return some dates that would have events
      return [`${year}-${String(month).padStart(2, '0')}-15`, `${year}-${String(month).padStart(2, '0')}-20`, `${year}-${String(month).padStart(2, '0')}-25`];
    }
    case 'event_list': return D.events.filter((e) => e.start >= a.from && e.start < a.to).sort((x, y) => x.start - y.start);
    case 'event_add': {
      const start = a.start; // Unix seconds directly
      const end = null;
      D.events.push({ id: ++seq, title: a.title, start, end, remind_min: a.remindMin, color: a.color ?? null, notified: false });
      return seq;
    }
    case 'event_update': {
      const e = D.events.find((x) => x.id === a.id);
      if (e) {
        const oldStart = e.start;
        const oldRemind = e.remind_min;
        e.title = a.title;
        e.start = a.start; // Unix seconds directly
        e.end = a.end;
        e.remind_min = a.remindMin;
        e.color = a.color;
        // Reset notified if start or remind_min changed
        if (oldStart !== e.start || oldRemind !== e.remind_min) {
          e.notified = 0;
        }
      }
      return;
    }
    case 'event_delete': return del('events', a.id);
    case 'clip_list': return D.clips
      .filter((c) => c.text.toLowerCase().includes((a.query || '').toLowerCase()))
      .sort((x, y) => y.pinned - x.pinned || y.ts - x.ts);
    case 'clip_copy': navigator.clipboard?.writeText(D.clips.find((c) => c.id === a.id)?.text ?? ''); return;
    case 'clip_pin': { const c = D.clips.find((x) => x.id === a.id); c.pinned = !c.pinned; return; }
    case 'clip_delete': return del('clips', a.id);
    case 'clip_clear': D.clips = D.clips.filter((c) => c.pinned); return;
    case 'clip_open_file': console.info('[mock] clip_open_file:', a.path); return;
    case 'clip_delete_file': { const idx = D.clips.findIndex((c) => c.file_path === a.path); if (idx >= 0) D.clips.splice(idx, 1); return; }
    case 'clip_paste_files': console.info('[mock] clip_paste_files'); return 1;
    case 'setting_get': return D.settings[a.key] ?? null;
    case 'auto_hide_get': return D.settings.auto_hide ?? false;
    case 'auto_hide_set': D.settings.auto_hide = a.enabled; return;
    case 'auto_hide_time_get': return D.settings.auto_hide_time ?? 30;
    case 'auto_hide_time_set': D.settings.auto_hide_time = a.seconds; return;
    case 'hide_fullscreen_get': return D.settings.hide_fullscreen ?? true;
    case 'hide_fullscreen_set': D.settings.hide_fullscreen = a.enabled; return;
    case 'hide_battery_saver_get': return D.settings.hide_battery_saver ?? false;
    case 'hide_battery_saver_set': D.settings.hide_battery_saver = a.enabled; return;
    case 'emit_settings_changed': return;
    case 'setting_set': D.settings[a.key] = a.value; return;
    case 'settings_get_all': return { ...D.settings };
    case 'open_settings': console.info('[mock] open_settings — no-op in browser'); return;
    case 'apply_window_settings': return; // no-op in browser
    case 'autostart_get': return false; // default to false in mock
    case 'autostart_set': return; // no-op in mock
    // progress API
    case 'progress_push':
      emit('progress://update', { value: a.value ?? 0, label: a.label ?? '' });
      return;
    case 'progress_clear':
      emit('progress://clear', {});
      return;
    case 'battery_get': return { charging: false, level: 0.72 };
    case 'open_native': console.info('[mock] open_native:', a.app); return;
    // -- workspace: network --
    case 'net_sampler_set': netGate(!!a.enabled); return;
    case 'net_snapshot': return lastNet;
    case 'net_settings_get': return {
      public: D.settings.net_ping_public === '1',
      host: D.settings.net_ping_host || '1.1.1.1',
      gateway: '192.168.1.1',
    };
    case 'net_settings_set': {
      if (a.public && !String(a.host || '').trim()) throw new Error('Enter a host to ping');
      D.settings.net_ping_public = a.public ? '1' : '0';
      D.settings.net_ping_host = String(a.host || '').trim() || '1.1.1.1';
      lastNet = null;
      return;
    }
    // -- workspace: digital wellbeing --
    case 'wellbeing_enabled_get': return D.settings.wellbeing_enabled === 'true';
    case 'wellbeing_enabled_set':
      D.settings.wellbeing_enabled = a.enabled ? 'true' : 'false';
      emit('wellbeing://changed', {});
      return;
    case 'wellbeing_today': return D.settings.wellbeing_enabled === 'false' ? [] : wellbeingRows();
    case 'wellbeing_range': return D.settings.wellbeing_enabled === 'false' ? [] : wellbeingRows();
    case 'wellbeing_clear': emit('wellbeing://changed', {}); return;
    // -- workspace: running apps / resources --
    case 'procs_set': appsGate(!!a.enabled); return;
    case 'procs_snapshot': return lastApps;
    // -- weather (opt-in; Open-Meteo on the Rust side) --
    case 'weather_get': return mockWeather();
    case 'weather_settings_get': return {
      enabled: D.settings.weather_enabled === 'true',
      place: D.settings.weather_place || '',
      units: D.settings.weather_units === 'f' ? 'f' : 'c',
      has_location: W.lat !== null,
    };
    case 'weather_enabled_set':
      D.settings.weather_enabled = a.enabled ? 'true' : 'false';
      if (a.enabled) emit('weather://update', mockWeather());
      return;
    case 'weather_units_set':
      D.settings.weather_units = String(a.units || 'c').startsWith('f') ? 'f' : 'c';
      emit('weather://update', mockWeather());
      return;
    case 'weather_location_set': return setMockPlace(a.query);
    case 'weather_refresh':
      W.cache = null;
      emit('weather://update', mockWeather());
      return mockWeather();
    // -- workspace: window macros --
    case 'topmost_list': return WINDOWS();
    case 'topmost_float': apps.keep.add(a.hwnd); return;
    case 'topmost_release': apps.keep.delete(a.hwnd); return;
    case 'topmost_float_last': return 'Tauri 2 — WebView2';
    case 'topmost_release_all': {
      const n = apps.keep.size;
      apps.keep.clear();
      return n;
    }
    case 'enumerate_apps': return [
      { id: null, name: 'Google Chrome', path: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe', icon: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe', icon_filename: null, target: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe', group: null, hidden: false },
      { id: null, name: 'Visual Studio Code', path: 'C:\\Program Files\\Microsoft VS Code\\Code.exe', icon: 'C:\\Program Files\\Microsoft VS Code\\Code.exe', icon_filename: null, target: 'C:\\Program Files\\Microsoft VS Code\\Code.exe', group: null, hidden: false },
      { id: null, name: 'Spotify', path: 'C:\\Program Files\\Spotify\\Spotify.exe', icon: 'C:\\Program Files\\Spotify\\Spotify.exe', icon_filename: null, target: 'C:\\Program Files\\Spotify\\Spotify.exe', group: null, hidden: false },
    ];
    case 'app_shortcuts_list': return D.appShortcuts || [];
    case 'app_shortcut_add': {
      if (!D.appShortcuts) D.appShortcuts = [];
      D.appShortcuts.push({ id: ++seq, name: a.name, path: a.path, icon: a.icon, icon_filename: null });
      return seq;
    }
    case 'app_shortcut_remove': {
      if (D.appShortcuts) D.appShortcuts = D.appShortcuts.filter(s => s.id !== a.id);
      return;
    }
    case 'app_shortcut_launch': console.info('[mock] app_shortcut_launch:', a.path); return;
    case 'app_shortcut_icon': {
      // Handle updated signature: iconFilename (required) and id (optional for self-heal)
      if (a.iconFilename) {
        console.info('[mock] app_shortcut_icon: iconFilename=', a.iconFilename, 'id=', a.id);
        return 'data:image/png;base64,';  // Mock base64 PNG
      }
      console.info('[mock] app_shortcut_icon called without iconFilename');
      return 'data:image/png;base64,';
    }
    case 'clear_all_icons': console.info('[mock] clear_all_icons'); return;
    case 'regenerate_shortcut_icon': console.info('[mock] regenerate_shortcut_icon'); return;
    case 'reextract_old_icons': console.info('[mock] reextract_old_icons'); return;
    case 'extract_app_icon': console.info('[mock] extract_app_icon:', a.path); return 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==';
    default: throw `mock: unknown command ${cmd}`;
  }
}

/* --------------------------------- dev helpers (browser only) ------------ */
if (typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window)) {
  window.isle = {
    /** isle.setProgress(42, 'Copying…') */
    setProgress: (v, label = '') => { emit('progress://update', { value: v, label }); },
    /** isle.clearProgress() */
    clearProgress: () => { emit('progress://clear', {}); },
    /** isle.simulateCharge(true/false, 0.85) */
    simulateCharge: (charging, level = 0.85) => { emit('battery://update', { charging, level }); },
    /** isle.weather('Mumbai') — set a mock location (browser only) */
    weather: (place = 'Mumbai') => setMockPlace(place),
    /** isle.demo() — runs a fake file-copy progress from 0→100 */
    demo: () => {
      let v = 0;
      const t = setInterval(() => {
        v += Math.random() * 6 + 2;
        if (v >= 100) { v = 100; clearInterval(t); }
        emit('progress://update', { value: Math.round(v), label: v < 100 ? 'Copying…' : 'Done!' });
      }, 150);
    },
  };
  console.info('[isle] Dev helpers available: window.isle.setProgress / clearProgress / simulateCharge / demo');
}
