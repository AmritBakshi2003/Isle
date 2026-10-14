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
const D = {
  settings: {
    wallpaper_dir: '', wallpaper_interval: '0', active_mode: '',
    // appearance
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
    module_clipboard: 'true', module_system: 'true',
    module_file_progress: 'true', module_download_progress: 'true',
    // media
    media_album_art: 'true', media_progress_ring: 'true',
    media_controls: 'full', media_update_speed: '1000',
    // progress
    progress_file_copy: 'true', progress_download: 'true',
    progress_app_install: 'true', progress_battery_stretch: 'true',
    progress_ring_style: 'arc', progress_auto_dismiss: '3',
    // performance
    power_mode: 'balanced', ui_update_rate: '1',
    blur_on_battery: 'true', hw_acceleration: 'true',
    // hotkeys
    hotkey_toggle: 'ctrl+alt+space', hotkey_settings: 'ctrl+alt+comma',
    // advanced
    log_level: 'error',
  },
  tasks: [
    { id: 1, title: 'Send the design review notes', done: false, due: null },
    { id: 2, title: 'Renew the domain', done: false, due: null },
    { id: 3, title: 'Book dentist', done: true, due: null },
  ],
  events: [
    { id: 1, title: 'Standup', start: at(10, 0), end: null, remind_min: 10 },
    { id: 2, title: 'Design review', start: at(15, 30), end: null, remind_min: 10 },
    { id: 3, title: 'Lunch with Asha', start: at(13, 0, 2), end: null, remind_min: 30 },
  ],
  clips: [
    { id: 1, text: 'https://tauri.app/start/', pinned: true, ts: sec() - 4000 },
    { id: 2, text: 'SELECT * FROM events WHERE starts > strftime(\'%s\',\'now\');', pinned: false, ts: sec() - 600 },
    { id: 3, text: 'Meeting moved to 3:30, same room.', pinned: false, ts: sec() - 90 },
  ],
  modes: [
    { id: 1, name: 'Focus', icon: '🎯', actions: [{ type: 'notify', title: 'Focus on', body: 'One thing at a time.' }] },
    { id: 2, name: 'Break', icon: '☕', actions: [{ type: 'notify', title: 'Break time', body: 'Step away.' }] },
  ],
  routines: [
    { id: 1, name: 'Morning setup', trigger: { kind: 'time', at: '09:00', days: [1, 2, 3, 4, 5] },
      actions: [{ type: 'wallpaper_next' }, { type: 'open', target: 'https://calendar.google.com' }], enabled: true },
  ],
};
const del = (k, id) => (D[k] = D[k].filter((x) => x.id !== id));

function mock(cmd, a) {
  switch (cmd) {
    case 'island_resize': return;
    case 'media_get': return { ...M, sampled_at_ms: Date.now() };
    case 'media_control':
      if (a.cmd === 'play_pause') { M.position_ms += Date.now() - M.sampled_at_ms; M.playing = !M.playing; }
      else { M.title = a.cmd === 'next' ? 'Glass Hours' : 'Night Drive'; M.position_ms = 0; M.art = art('#4cc9f0', '#3a0ca3'); }
      M.sampled_at_ms = Date.now();
      emit('media://update', { ...M });
      return;
    case 'task_list': return [...D.tasks].sort((x, y) => x.done - y.done || y.id - x.id);
    case 'task_add': D.tasks.push({ id: ++seq, title: a.title, done: false, due: a.due ?? null }); return seq;
    case 'task_toggle': { const t = D.tasks.find((x) => x.id === a.id); t.done = !t.done; return; }
    case 'task_delete': return del('tasks', a.id);
    case 'task_clear_done': D.tasks = D.tasks.filter((t) => !t.done); return;
    case 'event_list': return D.events.filter((e) => e.start >= a.from && e.start < a.to).sort((x, y) => x.start - y.start);
    case 'event_add': D.events.push({ id: ++seq, title: a.title, start: a.start, end: a.end ?? null, remind_min: a.remindMin }); return seq;
    case 'event_delete': return del('events', a.id);
    case 'clip_list': return D.clips
      .filter((c) => c.text.toLowerCase().includes((a.query || '').toLowerCase()))
      .sort((x, y) => y.pinned - x.pinned || y.ts - x.ts);
    case 'clip_copy': navigator.clipboard?.writeText(D.clips.find((c) => c.id === a.id)?.text ?? ''); return;
    case 'clip_pin': { const c = D.clips.find((x) => x.id === a.id); c.pinned = !c.pinned; return; }
    case 'clip_delete': return del('clips', a.id);
    case 'clip_clear': D.clips = D.clips.filter((c) => c.pinned); return;
    case 'mode_list': return D.modes;
    case 'mode_save':
      if (a.id) Object.assign(D.modes.find((m) => m.id === a.id), { name: a.name, icon: a.icon, actions: a.actions });
      else D.modes.push({ id: ++seq, name: a.name, icon: a.icon, actions: a.actions });
      return;
    case 'mode_delete': return del('modes', a.id);
    case 'mode_activate': D.settings.active_mode = String(a.id); return;
    case 'routine_list': return D.routines;
    case 'routine_save':
      if (a.id) Object.assign(D.routines.find((r) => r.id === a.id), { name: a.name, trigger: a.trigger, actions: a.actions });
      else D.routines.push({ id: ++seq, name: a.name, trigger: a.trigger, actions: a.actions, enabled: true });
      return;
    case 'routine_toggle': D.routines.find((r) => r.id === a.id).enabled = a.enabled; return;
    case 'routine_delete': return del('routines', a.id);
    case 'routine_run': return;
    case 'wallpaper_next': if (!D.settings.wallpaper_dir) throw 'Set a wallpaper folder first'; return;
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
