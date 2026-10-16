<script>
  import { onMount } from 'svelte';
  import { call, on } from '../lib/api.js';
  import Icon from '../lib/Icon.svelte';

  // -- default settings ------------------------------------------------------
  const DEFAULTS = {
    pill_size: 'medium', pill_position: 'center', corner_radius: '22',
    bg_color: 'rgba(26, 26, 26, 0)', bg_blur: 'auto', opacity: '95',
    border_on: 'false', border_color: '#6c6c6c',
    auto_hide: 'false', auto_hide_time: '30',
    hide_fullscreen: 'false', hide_battery_saver: 'false',
    click_action: 'expand', always_on_top: 'true',
    blur_on_battery: 'false', hover_expand: 'false',
    module_media: 'true', module_calendar: 'true', module_tasks: 'true',
    module_clipboard: 'true', module_system: 'true',
    module_file_progress: 'true', module_download_progress: 'true',
    media_album_art: 'true', media_progress_ring: 'true',
    media_controls: 'full', media_update_speed: '1000',
    progress_file_copy: 'true', progress_download: 'true',
    progress_app_install: 'true', progress_battery_stretch: 'true',
    progress_ring_style: 'arc', progress_auto_dismiss: '3',
    power_mode: 'balanced', ui_update_rate: '1',
    hw_acceleration: 'true',
    hotkey_toggle: 'ctrl+alt+space',
    log_level: 'error', log_to_file: 'false',
    single_instance_port: '47123',
  };

  let s = $state({ ...DEFAULTS, autostart: false });
  let tab = $state('appearance');
  let saved = $state(false);
  let recordingKey = $state(null);
  let loaded = $state(false);

  const TABS = [
    { id: 'appearance',   icon: 'appearance', label: 'Appearance' },
    { id: 'behavior',     icon: 'behavior',  label: 'Behavior' },
    { id: 'modules',      icon: 'modules', label: 'Modules' },
    { id: 'media',        icon: 'media', label: 'Media' },
    { id: 'progress',     icon: 'progress', label: 'Live Progress' },
    { id: 'performance',  icon: 'performance', label: 'Performance' },
    { id: 'hotkeys',      icon: 'hotkeys',  label: 'Hotkeys' },
    { id: 'advanced',     icon: 'advanced', label: 'Advanced' },
  ];

  onMount(async () => {
    try {
      const all = await call('settings_get_all');
      // Merge loaded settings with defaults, keeping defaults for missing keys
      Object.assign(s, DEFAULTS, all);
      loaded = true;
      s.autostart = await call('autostart_get');
    } catch(e) { console.warn('settings load:', e); loaded = true; }
  });

  async function save(key, value) {
    s[key] = value;
    try {
      await call('setting_set', { key, value: String(value) });
      // Emit Tauri event to notify main island to reload settings
      await call('emit_settings_changed');

      // Re-register hotkeys if hotkey settings changed
      if (key === 'hotkey_toggle') {
        await call('reregister_hotkeys');
      }

      // Reload logger if log_level or log_to_file changed
      if (key === 'log_level' || key === 'log_to_file') {
        await call('reload_logger');
      }

      // Enable/disable clipboard module
      if (key === 'module_clipboard') {
        await call('clipboard_enabled_set', { enabled: value === 'true' });
      }

      flash();
    } catch (e) {
      console.error('Failed to save setting:', key, e);
      alert(`Failed to save setting: ${e}`);
    }
  }

  async function saveAutostart(v) {
    s.autostart = v;
    try {
      await call('autostart_set', { enabled: v });
      flash();
    } catch (e) {
      console.error('Failed to save autostart:', e);
      alert(`Failed to save autostart: ${e}`);
    }
  }

  function flash() {
    saved = true;
    setTimeout(() => saved = false, 1800);
  }

  async function resetAll() {
    if (!confirm('Reset ALL settings to defaults?')) return;
    for (const [k, v] of Object.entries(DEFAULTS)) {
      await call('setting_set', { key: k, value: v });
    }
    Object.assign(s, DEFAULTS);
    s.autostart = false;
    await call('autostart_set', { enabled: false });
    flash();
  }

  async function exportSettings() {
    const data = JSON.stringify({ ...s, _version: '1' }, null, 2);
    const blob = new Blob([data], { type: 'application/json' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = 'isle-settings.json';
    a.click();
  }

  async function importSettings(e) {
    const file = e.target.files[0];
    if (!file) return;
    const text = await file.text();
    try {
      const data = JSON.parse(text);
      for (const [k, v] of Object.entries(data)) {
        if (k.startsWith('_')) continue;
        if (k === 'autostart') { await saveAutostart(v); continue; }
        await save(k, v);
      }
    } catch { alert('Invalid settings file.'); }
  }

  function startRecording(key) {
    recordingKey = key;
    const handler = (e) => {
      e.preventDefault();
      e.stopPropagation();
      const mods = [];
      if (e.ctrlKey) mods.push('ctrl');
      if (e.altKey) mods.push('alt');
      if (e.shiftKey) mods.push('shift');
      if (e.metaKey) mods.push('meta');
      const k = e.key.toLowerCase().replace(' ', 'space');
      if (['control','alt','shift','meta'].includes(k)) return;
      const combo = [...mods, k].join('+');
      console.log('Recording hotkey:', combo);
      save(key, combo);
      recordingKey = null;
      document.removeEventListener('keydown', handler, true);
    };
    document.addEventListener('keydown', handler, true);
  }

  const bool = (v) => v === 'true' || v === true;
</script>

{#if !loaded}
  <div class="loading">Loading settings…</div>
{:else}
<div class="settings-root">
  <!-- Sidebar -->
  <aside class="sidebar">
    <div class="logo">
      <Icon name="settings" size={20} />
      <span>Isle Settings</span>
    </div>
    {#each TABS as t}
      <button class="tab-btn" class:active={tab === t.id} onclick={() => tab = t.id}>
        <Icon name={t.icon} size={16} />
        <span>{t.label}</span>
      </button>
    {/each}
    <div class="sidebar-footer">
      {#if saved}<span class="saved-badge">✓ Saved</span>{/if}
    </div>
  </aside>

  <!-- Content -->
  <main class="content">

    <!-- -- APPEARANCE ---------------------------------------------------- -->
    {#if tab === 'appearance'}
    <section class="panel">
      <h2>Appearance</h2>

      <div class="row">
        <span class="row-label">Pill Size</span>
        <div class="seg">
          {#each ['small','medium','large'] as sz}
            <button class:on={s.pill_size===sz} onclick={()=>save('pill_size',sz)}>{sz}</button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="row-label">Position</span>
        <div class="seg">
          {#each ['left','center','right'] as p}
            <button class:on={s.pill_position===p} onclick={()=>save('pill_position',p)}>{p}</button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="row-label">Corner Radius <span class="val">{s.corner_radius}px</span></span>
        <input type="range" min="4" max="40" value={s.corner_radius}
               onchange={e=>save('corner_radius', e.target.value)} />
      </div>

      <div class="row">
        <span class="row-label">Background Color</span>
        <div class="color-row">
          <input type="color" value={s.bg_color} onchange={e=>save('bg_color',e.target.value)} />
          <span class="val">{s.bg_color}</span>
        </div>
      </div>

      <div class="row">
        <span class="row-label">Background Blur</span>
        <div class="seg">
          {#each ['off','on','auto'] as b}
            <button class:on={s.bg_blur===b} onclick={()=>save('bg_blur',b)}>{b}</button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="row-label">Opacity <span class="val">{s.opacity}%</span></span>
        <input type="range" min="30" max="100" value={s.opacity}
               onchange={e=>save('opacity',e.target.value)} />
      </div>

      <div class="row">
        <span class="row-label">Border</span>
        <div class="toggle-row">
          <button class="toggle" class:on={bool(s.border_on)}
                  onclick={()=>save('border_on', String(!bool(s.border_on)))}>
            {bool(s.border_on) ? 'On' : 'Off'}
          </button>
          {#if bool(s.border_on)}
            <input type="color" value={s.border_color}
                   onchange={e=>save('border_color',e.target.value)} />
          {/if}
        </div>
      </div>

      <!-- Live preview -->
      <div class="preview-wrap">
        <p class="hint">Preview</p>
        <div class="preview-pill" style="
          width: {s.pill_size==='small'?160:s.pill_size==='large'?240:200}px;
          border-radius: {s.corner_radius}px;
          background: {s.bg_color};
          opacity: {s.opacity/100};
          border: {bool(s.border_on) ? `1px solid ${s.border_color}` : 'none'};
          box-shadow: 0 4px 24px #0008;
          height: 38px;
        "></div>
      </div>
    </section>

    <!-- -- BEHAVIOR ------------------------------------------------------ -->
    {:else if tab === 'behavior'}
    <section class="panel">
      <h2>Behavior</h2>

      <div class="row">
        <span class="row-label">Start with Windows</span>
        <button class="toggle" class:on={s.autostart}
                onclick={()=>saveAutostart(!s.autostart)}>
          {s.autostart ? 'On' : 'Off'}
        </button>
      </div>

      <div class="row">
        <span class="row-label">Always on Top</span>
        <button class="toggle" class:on={bool(s.always_on_top)}
                onclick={()=>save('always_on_top', String(!bool(s.always_on_top)))}>
          {bool(s.always_on_top) ? 'On' : 'Off'}
        </button>
      </div>

      <div class="row">
        <span class="row-label">Auto Hide when Idle</span>
        <button class="toggle" class:on={bool(s.auto_hide)}
                onclick={()=>save('auto_hide', String(!bool(s.auto_hide)))}>
          {bool(s.auto_hide) ? 'On' : 'Off'}
        </button>
      </div>

      {#if bool(s.auto_hide)}
      <div class="row indent">
        <span class="row-label">Idle Time <span class="val">{s.auto_hide_time}s</span></span>
        <input type="range" min="5" max="300" value={s.auto_hide_time}
               onchange={e=>save('auto_hide_time',e.target.value)} />
      </div>
      {/if}

      <div class="row">
        <span class="row-label">Hide on Fullscreen App</span>
        <button class="toggle" class:on={bool(s.hide_fullscreen)}
                onclick={()=>save('hide_fullscreen', String(!bool(s.hide_fullscreen)))}>
          {bool(s.hide_fullscreen) ? 'On' : 'Off'}
        </button>
      </div>

      <div class="row">
        <span class="row-label">Hide on Battery Saver</span>
        <button class="toggle" class:on={bool(s.hide_battery_saver)}
                onclick={()=>save('hide_battery_saver', String(!bool(s.hide_battery_saver)))}>
          {bool(s.hide_battery_saver) ? 'On' : 'Off'}
        </button>
      </div>

      <div class="row">
        <span class="row-label">Click Action</span>
        <div class="seg">
          {#each ['expand','nothing'] as a}
            <button class:on={s.click_action===a} onclick={()=>save('click_action',a)}>{a}</button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="row-label">Expand on Hover</span>
        <button class="toggle" class:on={bool(s.hover_expand)}
                onclick={()=>save('hover_expand', String(!bool(s.hover_expand)))}>
          {bool(s.hover_expand) ? 'On' : 'Off'}
        </button>
      </div>
    </section>

    <!-- -- MODULES ------------------------------------------------------- -->
    {:else if tab === 'modules'}
    <section class="panel">
      <h2>Modules</h2>
      <p class="hint">Toggle which tabs appear in the island.</p>

      {#each [
        {key:'module_media',           label:'Media Player',          icon:'media'},
        {key:'module_calendar',        label:'Calendar & Events',      icon:'calendar'},
        {key:'module_tasks',           label:'Tasks',                  icon:'tasks'},
        {key:'module_clipboard',       label:'Clipboard History',      icon:'clips'},
        {key:'module_system',          label:'System (Battery/Wi-Fi)', icon:'system'},
        {key:'module_file_progress',   label:'File Copy Progress',     icon:'file_progress'},
        {key:'module_download_progress',label:'Download Progress',     icon:'download_progress'},
      ] as m}
      <div class="module-row">
        <div class="module-info">
          <Icon name={m.icon} size={18} />
          <span class="mod-label">{m.label}</span>
        </div>
        <button class="toggle" class:on={bool(s[m.key])}
                onclick={()=>save(m.key, String(!bool(s[m.key])))}>
          {bool(s[m.key]) ? 'On' : 'Off'}
        </button>
      </div>
      {/each}
    </section>

    <!-- -- MEDIA -------------------------------------------------------- -->
    {:else if tab === 'media'}
    <section class="panel">
      <h2>Media Settings</h2>

      <div class="row">
        <span class="row-label">Show Album Art</span>
        <button class="toggle" class:on={bool(s.media_album_art)}
                onclick={()=>save('media_album_art', String(!bool(s.media_album_art)))}>
          {bool(s.media_album_art) ? 'On' : 'Off'}
        </button>
      </div>

      <div class="row">
        <span class="row-label">Show Progress Ring</span>
        <button class="toggle" class:on={bool(s.media_progress_ring)}
                onclick={()=>save('media_progress_ring', String(!bool(s.media_progress_ring)))}>
          {bool(s.media_progress_ring) ? 'On' : 'Off'}
        </button>
      </div>

      <div class="row">
        <span class="row-label">Controls Style</span>
        <div class="seg">
          <button class:on={s.media_controls==='full'} onclick={()=>save('media_controls','full')}>Prev / Play / Next</button>
          <button class:on={s.media_controls==='play_only'} onclick={()=>save('media_controls','play_only')}>Play Only</button>
        </div>
      </div>

      <div class="row">
        <span class="row-label">Update Speed</span>
        <div class="seg">
          {#each [{v:'500',l:'0.5s'},{v:'1000',l:'1s'},{v:'2000',l:'2s'},{v:'3000',l:'3s'}] as sp}
            <button class:on={s.media_update_speed===sp.v} onclick={()=>save('media_update_speed',sp.v)}>{sp.l}</button>
          {/each}
        </div>
      </div>
    </section>

    <!-- -- LIVE PROGRESS ------------------------------------------------ -->
    {:else if tab === 'progress'}
    <section class="panel">
      <h2>Live Progress</h2>

      {#each [
        {key:'progress_file_copy',       label:'File Copy Bar'},
        {key:'progress_download',        label:'Download Bar (BITS)'},
        {key:'progress_app_install',     label:'App Install Bar'},
        {key:'progress_battery_stretch', label:'Battery Stretch Animation'},
      ] as p}
      <div class="row">
        <span class="row-label">{p.label}</span>
        <button class="toggle" class:on={bool(s[p.key])}
                onclick={()=>save(p.key, String(!bool(s[p.key])))}>
          {bool(s[p.key]) ? 'On' : 'Off'}
        </button>
      </div>
      {/each}

      <div class="row">
        <span class="row-label">Ring Style</span>
        <div class="seg">
          {#each ['arc','full','bar'] as rs}
            <button class:on={s.progress_ring_style===rs} onclick={()=>save('progress_ring_style',rs)}>{rs}</button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="row-label">Auto Dismiss <span class="val">{s.progress_auto_dismiss}s after complete</span></span>
        <input type="range" min="1" max="10" value={s.progress_auto_dismiss}
               onchange={e=>save('progress_auto_dismiss',e.target.value)} />
      </div>
    </section>

    <!-- -- PERFORMANCE -------------------------------------------------- -->
    {:else if tab === 'performance'}
    <section class="panel">
      <h2>Performance</h2>
      <p class="hint">Optimised defaults for i5-class laptops.</p>

      <div class="row">
        <span class="row-label">Power Mode</span>
        <div class="seg">
          {#each ['eco','balanced','performance'] as pm}
            <button class:on={s.power_mode===pm} onclick={()=>save('power_mode',pm)}>{pm}</button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="row-label">UI Update Rate</span>
        <div class="seg">
          {#each [{v:'1',l:'1�/s'},{v:'2',l:'2�/s'},{v:'4',l:'4�/s'}] as r}
            <button class:on={s.ui_update_rate===r.v} onclick={()=>save('ui_update_rate',r.v)}>{r.l}</button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="row-label">Disable Blur on Battery</span>
        <button class="toggle" class:on={bool(s.blur_on_battery)}
                onclick={()=>save('blur_on_battery', String(!bool(s.blur_on_battery)))}>
          {bool(s.blur_on_battery) ? 'On' : 'Off'}
        </button>
      </div>

      <div class="row">
        <span class="row-label">Hardware Acceleration</span>
        <button class="toggle" class:on={bool(s.hw_acceleration)}
                onclick={()=>save('hw_acceleration', String(!bool(s.hw_acceleration)))}>
          {bool(s.hw_acceleration) ? 'On' : 'Off'}
        </button>
      </div>

      <div class="info-box">
        <strong>Eco mode:</strong> halves UI update rate and disables blur.<br/>
        <strong>Performance mode:</strong> enables 4�/s updates and all visual effects.<br/>
        Changes apply after restarting Isle.
      </div>
    </section>

    <!-- -- HOTKEYS ------------------------------------------------------- -->
    {:else if tab === 'hotkeys'}
    <section class="panel">
      <h2>Hotkeys</h2>
      <p class="hint">Click a key field, then press your desired shortcut.</p>

      {#each [
        {key:'hotkey_toggle',   label:'Expand / Collapse Island'},
      ] as hk}
      <div class="row">
        <span class="row-label">{hk.label}</span>
        <button class="key-badge"
                class:recording={recordingKey===hk.key}
                onclick={()=>startRecording(hk.key)}>
          {recordingKey===hk.key ? '? Press keys�' : (s[hk.key] || 'Not set')}
        </button>
      </div>
      {/each}

      <div class="info-box">
        Hotkey changes take effect immediately.<br/>
        Current toggle: <strong>{s.hotkey_toggle}</strong><br/>
        Open Settings from the island by clicking the Settings tab.
      </div>
    </section>

    <!-- -- ADVANCED ------------------------------------------------------ -->
    {:else if tab === 'advanced'}
    <section class="panel">
      <h2>Advanced</h2>

      <div class="row">
        <span class="row-label">Log Level</span>
        <div class="seg">
          {#each ['none','error','info'] as l}
            <button class:on={s.log_level===l} onclick={()=>save('log_level',l)}>{l}</button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="row-label">Log to File</span>
        <div class="seg">
          {#each ['false','true'] as v}
            <button class:on={s.log_to_file===v} onclick={()=>save('log_to_file',v)}>{v==='true'?'On':'Off'}</button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="row-label">Single Instance Port</span>
        <input type="number" value={s.single_instance_port}
               onchange={e=>save('single_instance_port',e.target.value)} />
      </div>

      <div class="divider"></div>

      <div class="action-row">
        <div>
          <strong>Export Settings</strong>
          <p>Save all settings to a JSON file.</p>
        </div>
        <button class="btn-action" onclick={exportSettings}>Export</button>
      </div>

      <div class="action-row">
        <div>
          <strong>Import Settings</strong>
          <p>Load settings from a JSON file.</p>
        </div>
        <label class="btn-action import-label">
          Import
          <input type="file" accept=".json" onchange={importSettings} style="display:none" />
        </label>
      </div>

      <div class="divider"></div>

      <div class="action-row danger">
        <div>
          <strong>Reset to Defaults</strong>
          <p>Wipe all settings and restore factory defaults.</p>
        </div>
        <button class="btn-action danger" onclick={resetAll}>Reset</button>
      </div>
    </section>
    {/if}

  </main>
</div>
{/if}

<style>
  :global(*, *::before, *::after) { box-sizing: border-box; margin: 0; padding: 0; }
  :global(body) {
    background: #0d0d0f;
    color: #e8e8ea;
    font-family: 'Segoe UI', system-ui, sans-serif;
    font-size: 14px;
    height: 100vh;
    overflow: hidden;
  }

  .loading {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    color: #666;
    font-size: 14px;
  }

  .settings-root {
    display: flex;
    height: 100vh;
  }

  /* -- Sidebar -- */
  .sidebar {
    width: 192px;
    background: #111115;
    border-right: 1px solid #1f1f24;
    display: flex;
    flex-direction: column;
    padding: 16px 0;
    flex-shrink: 0;
  }
  .logo {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 20px 20px;
    font-weight: 700;
    font-size: 15px;
    color: #fff;
    letter-spacing: -.3px;
  }
  .tab-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 20px;
    background: none;
    border: none;
    color: #888;
    cursor: pointer;
    font-size: 13.5px;
    border-radius: 0;
    transition: all .15s;
    text-align: left;
    width: 100%;
  }
  .tab-btn:hover { background: #18181e; color: #ccc; }
  .tab-btn.active { background: #1e1e28; color: #fff; font-weight: 600; }
  .tab-btn.active::before {
    content: '';
    position: absolute;
    left: 0;
    width: 3px;
    height: 36px;
    background: #7c6af7;
    border-radius: 0 2px 2px 0;
  }
  .tab-btn { position: relative; }
  .sidebar-footer {
    margin-top: auto;
    padding: 16px 20px;
    min-height: 40px;
  }
  .saved-badge {
    font-size: 12px;
    color: #4caf7d;
    font-weight: 600;
    animation: fadein .2s;
  }
  @keyframes fadein { from { opacity: 0; } to { opacity: 1; } }

  /* -- Content -- */
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 32px 36px;
    scrollbar-width: thin;
    scrollbar-color: #2a2a30 transparent;
  }

  .panel h2 {
    font-size: 22px;
    font-weight: 700;
    margin-bottom: 28px;
    color: #f0f0f2;
    letter-spacing: -.4px;
  }

  .hint {
    color: #666;
    font-size: 12.5px;
    margin-top: -18px;
    margin-bottom: 22px;
  }

  /* -- Rows -- */
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 0;
    border-bottom: 1px solid #18181e;
    gap: 16px;
  }
  .row.indent { padding-left: 20px; background: #0e0e12; border-radius: 6px; }
  .row .row-label {
    color: #c0c0c8;
    font-size: 13.5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 170px;
  }
  .val { color: #666; font-size: 11.5px; font-weight: 400; }

  /* -- Inputs -- */
  input[type=range] {
    flex: 1;
    accent-color: #7c6af7;
    height: 4px;
    cursor: pointer;
  }

  /* -- Segmented control -- */
  .seg {
    display: flex;
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid #2a2a32;
  }
  .seg button {
    background: #18181e;
    border: none;
    color: #888;
    padding: 6px 14px;
    cursor: pointer;
    font-size: 12.5px;
    transition: all .12s;
    border-right: 1px solid #2a2a32;
    white-space: nowrap;
  }
  .seg button:last-child { border-right: none; }
  .seg button:hover { background: #222228; color: #ccc; }
  .seg button.on { background: #7c6af7; color: #fff; font-weight: 600; }

  /* -- Toggle -- */
  .toggle {
    border: none;
    border-radius: 20px;
    padding: 6px 18px;
    font-size: 12.5px;
    cursor: pointer;
    background: #222228;
    color: #888;
    min-width: 56px;
    transition: all .15s;
    font-weight: 600;
  }
  .toggle.on { background: #7c6af7; color: #fff; }

  /* -- Color row -- */
  .color-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  input[type=color] {
    width: 36px; height: 36px;
    border: 2px solid #2a2a32;
    border-radius: 8px;
    cursor: pointer;
    background: none;
    padding: 2px;
  }
  .toggle-row { display: flex; align-items: center; gap: 10px; }

  /* -- Module rows -- */
  .module-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 0;
    border-bottom: 1px solid #18181e;
  }
  .module-info { display: flex; align-items: center; gap: 12px; }
  .mod-label { color: #c0c0c8; font-size: 13.5px; }

  /* -- Preview pill -- */
  .preview-wrap { margin-top: 28px; }
  .preview-pill { transition: all .3s; }

  /* -- Key badge -- */
  .key-badge {
    background: #18181e;
    border: 1px solid #2a2a32;
    border-radius: 8px;
    color: #aaa;
    padding: 8px 16px;
    font-size: 13px;
    cursor: pointer;
    font-family: 'Consolas', monospace;
    letter-spacing: .5px;
    min-width: 180px;
    text-align: center;
    transition: all .15s;
  }
  .key-badge:hover { border-color: #7c6af7; color: #fff; }
  .key-badge.recording {
    border-color: #f7706a;
    color: #f7706a;
    animation: pulse-rec 1s infinite;
  }
  @keyframes pulse-rec {
    0%, 100% { box-shadow: 0 0 0 0 #f7706a44; }
    50% { box-shadow: 0 0 0 6px #f7706a00; }
  }

  /* -- Info box -- */
  .info-box {
    background: #14141a;
    border: 1px solid #1e1e28;
    border-radius: 10px;
    padding: 14px 18px;
    color: #666;
    font-size: 12.5px;
    line-height: 1.6;
    margin-top: 20px;
  }

  /* -- Action rows (advanced) -- */
  .divider { border-top: 1px solid #1a1a22; margin: 20px 0; }
  .action-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 0;
    border-bottom: 1px solid #18181e;
    gap: 16px;
  }
  .action-row p { color: #666; font-size: 12px; margin-top: 3px; }
  .btn-action {
    border: none;
    border-radius: 8px;
    padding: 8px 20px;
    background: #252530;
    color: #ccc;
    font-size: 13px;
    cursor: pointer;
    font-weight: 600;
    transition: all .15s;
    white-space: nowrap;
  }
  .btn-action:hover { background: #7c6af7; color: #fff; }
  .btn-action.danger { background: #2e1a1a; color: #f7706a; }
  .btn-action.danger:hover { background: #f7706a; color: #fff; }
  .action-row.danger strong { color: #f7706a; }
  .import-label { display: inline-block; }
</style>
