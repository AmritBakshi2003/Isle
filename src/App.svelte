<!--
  App.svelte: the island shell. Owns expand/collapse, tabs, global events.

  Expand sequence (order matters, see src-tauri/src/island.rs):
    expand():   invoke island_resize(true) -> set ui.expanded -> CSS morphs the pill
    collapse(): set ui.expanded=false -> wait for the morph -> island_resize(false)
  Collapse triggers: Esc, window blur (click elsewhere), hotkey toggle, pill click when open? (no).
  Sizes in app.css/.island MUST match COLLAPSED/EXPANDED in island.rs.

  Live Progress additions:
    - SVG progress ring lives *outside* .island overflow:hidden, absolutely centred on the pill.
    - .island.charging widens the pill by 16px (CSS transition handles spring).
    - Battery dot and bolt icon fade in/out as charging state changes.
    - Subscribes to progress://update, progress://clear, battery://update events.
-->
<script>
  import { onMount } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { call, on, inTauri } from './lib/api.js';
  import { ui, media, progress, setProgress, clearProgress, applyMedia, accentFromArt } from './lib/state.svelte.js';
  import { fmtClock } from './lib/util.js';
  import Icon from './lib/Icon.svelte';
  import Music from './views/Music.svelte';
  import Clips from './views/Clips.svelte';
  import TaskCalendar from './views/TaskCalendar.svelte';
  import UtilityDrawer from './views/UtilityDrawer.svelte';
  import Workspace from './views/Workspace.svelte';

  const allTabs = [
    { id: 'music', label: 'Music', icon: 'music', setting: 'module_media' },
    { id: 'clips', label: 'Clipboard', icon: 'clips', setting: 'module_clipboard' },
    { id: 'tasks', label: 'Tasks & Calendar', icon: 'tasks', setting: 'module_tasks' },
    { id: 'utility', label: 'Utility Drawer', icon: 'utility', setting: null },
    { id: 'workspace', label: 'Workspace', icon: 'workspace', setting: 'module_workspace' },
    { id: 'settings', label: 'Settings', icon: 'settings', setting: null },
  ];
  
  let moduleSettings = $state({
    module_media: 'true',
    module_clipboard: 'true',
    module_tasks: 'true',
    module_calendar: 'true',
    module_workspace: 'true',
    click_action: 'expand',
    blur_on_battery: 'false',
    bg_blur: 'auto',
    media_album_art: 'true',
    media_progress_ring: 'true',
    progress_file_copy: 'true',
    progress_download: 'false',
    progress_app_install: 'false',
    progress_battery_stretch: 'true',
    progress_ring_style: 'arc',
    progress_auto_dismiss: '3',
    hover_expand: 'false',
  });
  
  const tabs = $derived(allTabs.filter(t => {
    if (!t.setting) return true;
    // one tab holds two modules: it stays while either is enabled
    if (t.id === 'tasks') {
      const on = (v) => v === 'true' || v === true;
      return on(moduleSettings.module_tasks) || on(moduleSettings.module_calendar);
    }
    const val = moduleSettings[t.setting];
    return val === 'true' || val === true;
  }));
  const index = $derived(tabs.findIndex((t) => t.id === ui.tab));

  let clock = $state(fmtClock());
  let closeTimer;
  let autoDismissTimer;
  let hoverTimer;
  let chargingIndicatorTimer;
  let t;
  let settingsLoaded = $state(false);
  let isHovering = $state(false);
  let showChargingIndicator = $state(false);

  // ── weather chip (opt-in; backend in src-tauri/src/weather.rs) ─────────
  // Cached on the backend, so a cold start paints instantly and no request
  // happens unless the user set a place in Settings → Weather.
  let weather = $state(null);

  /** Collapsed-pill content: while a track is playing the pill shows the album
   *  art + equaliser instead of the clock — and the weather chip is dropped
   *  too, so only the music animation shows (weather returns with the clock). */
  const showMusic = $derived(media.active && media.playing);

  const weatherTemp = $derived.by(() => {
    if (!weather?.ok) return "";
    const c = Number(weather.temp_c) || 0;
    return `${Math.round(weather.units === "f" ? (c * 9) / 5 + 32 : c)}°`;
  });

  /** WMO weather code -> icon name (see Icon.svelte). */
  function weatherIconFor(code) {
    const c = Number(code);
    if (c === 0 || c === 1) return "sun";
    if (c === 2) return "cloud-sun";
    if (c === 3) return "cloud";
    if (c === 45 || c === 48) return "fog";
    if ((c >= 51 && c <= 57) || (c >= 61 && c <= 67) || (c >= 80 && c <= 82)) return "rain";
    if ((c >= 71 && c <= 77) || c === 85 || c === 86) return "snow";
    if (c >= 95) return "storm";
    return "cloud-sun";
  }
  const weatherIcon = $derived(weather?.ok ? weatherIconFor(weather.code) : "sun");
  const weatherTitle = $derived.by(() => {
    if (!weather?.ok) return "Weather: set a location in Settings";
    const f = weather.units === "f";
    const feels = Math.round(f ? (Number(weather.feels_c) * 9) / 5 + 32 : Number(weather.feels_c));
    return `${weather.place} · feels ${feels}° · ${weather.humidity}% humidity · ${Math.round(Number(weather.wind_kph) || 0)} km/h`;
  });

  // ── apply appearance + behavior settings ───────────────────────────────
  const DEFAULT_SETTINGS = {
    corner_radius: '22',
    opacity: '95',
    bg_color: 'rgba(26, 26, 26, 0.95)',
    bg_blur: 'auto',
    border_on: 'false',
    border_color: '#6c6c6c',
    pill_size: 'medium',
    pill_position: 'center',
    always_on_top: 'true',
    click_action: 'expand',
    module_media: 'true',
    module_clipboard: 'true',
    module_tasks: 'true',
    module_calendar: 'true',
    blur_on_battery: 'false',
    media_album_art: 'true',
    media_progress_ring: 'true',
    progress_file_copy: 'true',
    progress_download: 'false',
    progress_app_install: 'false',
    progress_battery_stretch: 'true',
    progress_ring_style: 'arc',
    progress_auto_dismiss: '3',
    hover_expand: 'false',
  };

  function applySettings(s) {
    if (!s) return;
    
    // Fix: if bg_color is fully transparent (alpha 0), use default instead
    if (s.bg_color && s.bg_color.includes(', 0)')) {
      s.bg_color = DEFAULT_SETTINGS.bg_color;
    }
    
    // Fix: if opacity is 0, use default instead
    if (s.opacity && Number(s.opacity) === 0) {
      s.opacity = DEFAULT_SETTINGS.opacity;
    }
    
    // Merge with defaults for missing keys
    const settings = { ...DEFAULT_SETTINGS, ...s };
    
    // Update module settings from loaded settings
    Object.assign(moduleSettings, {
      module_media: settings.module_media ?? 'true',
      module_clipboard: settings.module_clipboard ?? 'true',
      module_tasks: settings.module_tasks ?? 'true',
      module_calendar: settings.module_calendar ?? 'true',
      module_workspace: settings.module_workspace ?? 'true',
      click_action: settings.click_action ?? 'expand',
      blur_on_battery: settings.blur_on_battery ?? 'false',
      bg_blur: settings.bg_blur ?? 'auto',
      media_album_art: settings.media_album_art ?? 'true',
      media_progress_ring: settings.media_progress_ring ?? 'true',
      progress_file_copy: settings.progress_file_copy ?? 'true',
      progress_download: settings.progress_download ?? 'false',
      progress_app_install: settings.progress_app_install ?? 'false',
      progress_battery_stretch: settings.progress_battery_stretch ?? 'true',
      progress_ring_style: settings.progress_ring_style ?? 'arc',
      progress_auto_dismiss: settings.progress_auto_dismiss ?? '3',
      hover_expand: settings.hover_expand ?? 'false',
    });
    
    const root = document.documentElement;
    
    // Apply other settings
    // ── Appearance ─
    if (settings.corner_radius) root.style.setProperty('--setting-radius', settings.corner_radius + 'px');
    if (settings.opacity) root.style.setProperty('--setting-opacity', String(Number(settings.opacity) / 100));
    if (settings.bg_color) root.style.setProperty('--setting-bg', settings.bg_color);
    root.style.setProperty('--setting-border',
      settings.border_on === 'true' ? `1px solid ${settings.border_color || '#fff'}` : 'none');
    
    // Background blur
    root.style.setProperty('--setting-blur', settings.bg_blur || 'auto');

    // pill size → collapsed pill width CSS var
    const sizeMap = { small: '170px', medium: '200px', large: '240px' };
    root.style.setProperty('--setting-pill-w', sizeMap[settings.pill_size] || '200px');

    // pill position → stage justify-content CSS var
    const posMap = { left: 'flex-start', center: 'center', right: 'flex-end' };
    root.style.setProperty('--setting-pos', posMap[settings.pill_position] || 'center');

    // ── Behavior ──
    // always_on_top: tell backend to update the window property
    if (inTauri && settings.always_on_top !== undefined) {
      call('apply_window_settings', { alwaysOnTop: settings.always_on_top === 'true' }).catch(() => {});
    }

    settingsLoaded = true;
  }
  
  // Update tab count CSS variable when tabs change
  $effect(() => {
    const root = document.documentElement;
    root.style.setProperty('--tab-count', String(tabs.length));
  });
  
  // Update blur based on battery state when blur_on_battery is enabled
  $effect(() => {
    const root = document.documentElement;
    if (moduleSettings.blur_on_battery === 'true' && progress.battery !== null && !progress.charging) {
      root.style.setProperty('--setting-blur', 'none');
    } else {
      root.style.setProperty('--setting-blur', moduleSettings.bg_blur || 'auto');
    }
  });

  // Show charging indicator temporarily when charging state changes to true
  $effect(() => {
    if (progress.charging && !showChargingIndicator) {
      showChargingIndicator = true;
      clearTimeout(chargingIndicatorTimer);
      chargingIndicatorTimer = setTimeout(() => {
        showChargingIndicator = false;
      }, 3000); // Show for 3 seconds then hide
    }
    return () => clearTimeout(chargingIndicatorTimer);
  });

  // Auto-dismiss progress when it reaches 100%
  $effect(() => {
    if (progress.value >= 100 && Number(moduleSettings.progress_auto_dismiss) > 0) {
      clearTimeout(autoDismissTimer);
      autoDismissTimer = setTimeout(() => {
        clearProgress();
      }, Number(moduleSettings.progress_auto_dismiss) * 1000);
    }
    return () => clearTimeout(autoDismissTimer);
  });

  // ── progress ring geometry ─────────────────────────────────────────────
  // Ring is drawn in an SVG sibling to .island; sits ~7px outside the pill edge.
  const RING_PAD    = 7;    // gap between pill edge and ring stroke centre
  const RING_STROKE = 3;
  const PILL_W      = $derived(() => {
    const sizeMap = { small: 170, medium: 200, large: 240 };
    const baseW = sizeMap[moduleSettings.pill_size] || 200;
    return !ui.expanded && showChargingIndicator ? baseW + 16 : baseW;
  });
  const PILL_H      = 38;
  const ringR       = $derived(PILL_H / 2 + RING_PAD);
  const svgW        = $derived(PILL_W() + (RING_PAD + RING_STROKE + 2) * 2);
  const svgH        = PILL_H + (RING_PAD + RING_STROKE + 2) * 2;
  const cx          = $derived(svgW / 2);
  const cy          = svgH / 2;
  const circ        = $derived(2 * Math.PI * ringR);
  const dashOff     = $derived(progress.value >= 0 ? circ * (1 - progress.value / 100) : circ);

  // Ring style: arc (default), full, bar
  const ringStyle = $derived(moduleSettings.progress_ring_style || 'arc');
  const strokeDasharray = $derived(ringStyle === 'full' ? circ : ringStyle === 'bar' ? circ * 0.5 : circ);
  const dashOffset    = $derived(ringStyle === 'bar' ? circ * 0.25 : dashOff);

  // battery dot colour
  const battColour  = $derived(
    progress.battery === null ? 'transparent' :
    progress.battery > 0.5   ? '#4ade80' :
    progress.battery > 0.2   ? '#facc15' : '#f87171'
  );
  
  // Battery-based blur
  const shouldDisableBlur = $derived(
    moduleSettings.blur_on_battery === 'true' && progress.battery !== null && !progress.charging
  );

  // ── expand / collapse ──────────────────────────────────────────────────
  async function expand() {
    if (ui.expanded) return;
    clearTimeout(closeTimer);
    await call('island_resize', { expanded: true });
    ui.expanded = true;
    call('media_get').then(applyMedia);
  }

  function collapse() {
    if (!ui.expanded) return;
    ui.expanded = false;
    clearTimeout(closeTimer);
    closeTimer = setTimeout(() => call('island_resize', { expanded: false }), 520);
  }
  const toggle = () => (ui.expanded ? collapse() : expand());

  // Show pill without expanding (for hotkey)
  function showPill() {
    if (ui.expanded) {
      collapse();
    }
    // Ensure window is visible and focused
    if (inTauri) {
      call('window_show').catch(() => {});
    }
  }
  
  function handlePillClick() {
    const action = moduleSettings.click_action || 'expand';
    if (action === 'expand') {
      toggle();
    }
    // If action is 'nothing', do nothing
  }

  // ── hover expand/collapse ─────────────────────────────────────────────────
  function handleMouseEnter() {
    if (moduleSettings.hover_expand === 'true' && !ui.expanded) {
      clearTimeout(hoverTimer);
      hoverTimer = setTimeout(() => {
        expand();
      }, 100); // Small delay to prevent accidental expansion
    }
    isHovering = true;
  }

  function handleMouseLeave() {
    isHovering = false;
    if (moduleSettings.hover_expand === 'true' && ui.expanded) {
      clearTimeout(hoverTimer);
      hoverTimer = setTimeout(() => {
        collapse();
      }, 300); // Delay before collapsing when mouse leaves
    }
  }

  function openSettings() {

    call('open_settings').then(() => {

    }).catch((err) => {
      console.error('Failed to open settings:', err);
    });
  }

  function handleTabClick(tabId) {

    if (tabId === 'settings') {
      openSettings();
      // Switch back to music tab after opening settings
      ui.tab = 'music';
    } else {
      ui.tab = tabId;

    }
  }

  function onKey(e) {
    if (e.key === 'Escape') collapse();
    else if (ui.expanded && e.altKey && /^[1-6]$/.test(e.key)) ui.tab = tabs[+e.key - 1].id;
  }

  $effect(() => {
    const art = media.art;
    accentFromArt(art).then((c) => (ui.accent = c));
  });

  // ── battery ────────────────────────────────────────────────────────────
  // In Tauri mode: the Rust progress.rs watcher pushes battery://update events
  // automatically via Battery::ReportUpdated — zero polling, zero CPU.
  // In browser dev mode only: fall back to navigator.getBattery().
  async function refreshBattery() {
    if (inTauri) return; // Rust watcher handles it via battery://update events
    if (typeof navigator !== 'undefined' && navigator.getBattery) {
      try {
        const b = await navigator.getBattery();
        progress.charging = b.charging;
        progress.battery  = b.level;
        // Wire live events for the browser session.
        b.onchargingchange = () => { progress.charging = b.charging; };
        b.onlevelchange    = () => { progress.battery  = b.level;    };
      } catch { /* not supported */ }
    }
  }

  onMount(() => {
    if (!inTauri) document.documentElement.style.background = 'linear-gradient(135deg,#2a3142,#4b3b52)';
    call('media_get').then(applyMedia);

    call('weather_get').then((w) => (weather = w)).catch(() => {});

    const offs = [
      on('media://update', applyMedia),
      on('weather://update', (w) => (weather = w)),
      on('island://toggle', toggle),
      on('island://show', showPill),
      on('progress://update', ({ value, label }) => {
        setProgress(value, label ?? '');
      }),
      on('progress://clear',  ()                 => clearProgress()),
      on('battery://update',  ({ charging, level }) => {
        progress.charging = charging;
        progress.battery  = level;
      }),
      on('settings://changed', () => {
        call('settings_get_all').then((all) => {
          applySettings(all);
          // Also update media and progress-specific settings
          if (all.media_album_art !== undefined) moduleSettings.media_album_art = all.media_album_art;
          if (all.media_progress_ring !== undefined) moduleSettings.media_progress_ring = all.media_progress_ring;
          if (all.progress_file_copy !== undefined) moduleSettings.progress_file_copy = all.progress_file_copy;
          if (all.progress_download !== undefined) moduleSettings.progress_download = all.progress_download;
          if (all.progress_app_install !== undefined) moduleSettings.progress_app_install = all.progress_app_install;
          if (all.progress_battery_stretch !== undefined) moduleSettings.progress_battery_stretch = all.progress_battery_stretch;
          if (all.progress_ring_style !== undefined) moduleSettings.progress_ring_style = all.progress_ring_style;
          if (all.progress_auto_dismiss !== undefined) moduleSettings.progress_auto_dismiss = all.progress_auto_dismiss;
        }).catch(() => {});
      }),
      on('system://hide', (shouldHide) => {
        if (inTauri) {
          if (shouldHide) {
            call('window_hide').catch(() => {});
          } else {
            call('window_show').catch(() => {});
          }
        }
      }),
    ];
    const onBlur = () => inTauri && collapse();
    window.addEventListener('blur', onBlur);

    const tick = () => {
      clock = fmtClock();
      // Only poll battery in browser dev mode.
      // In Tauri mode battery://update events arrive from the Rust watcher.
      if (!inTauri) refreshBattery();
      t = setTimeout(tick, 60000 - (Date.now() % 60000) + 50);
    };
    tick();

    // Load settings and apply CSS vars on mount
    call('settings_get_all').then(applySettings).catch(() => {});

    return () => {
      offs.forEach((o) => o());
      window.removeEventListener('blur', onBlur);
      clearTimeout(t);
      clearTimeout(hoverTimer);
      clearTimeout(chargingIndicatorTimer);
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<main class="stage" style:--accent={ui.accent}>

  <!-- ── progress ring ── lives outside .island so it's never clipped ── -->
  {#if progress.value >= 0 && !ui.expanded && moduleSettings.media_progress_ring === 'true'}
    <svg
      class="progress-ring"
      width={svgW} height={svgH}
      viewBox="0 0 {svgW} {svgH}"
      aria-hidden="true"
      in:fade={{ duration: 220 }}
      out:fade={{ duration: 350 }}
      style="left: 50%; transform: translateX(-50%)"
    >
      {#if ringStyle === 'arc' || ringStyle === 'full'}
        <circle cx={cx} cy={cy} r={ringR} fill="none"
          stroke="rgba(255,255,255,0.1)" stroke-width={RING_STROKE} />
        <circle cx={cx} cy={cy} r={ringR} fill="none"
          stroke={progress.value >= 100 ? '#4ade80' : 'var(--accent)'}
          stroke-width={RING_STROKE} stroke-linecap="round"
          stroke-dasharray={strokeDasharray} stroke-dashoffset={dashOffset}
          transform="rotate(-90 {cx} {cy})"
          class="arc"
        />
      {:else if ringStyle === 'bar'}
        <rect x={cx - ringR} y={cy - 4} width={ringR * 2} height={8} rx={4}
          fill="rgba(255,255,255,0.1)" />
        <rect x={cx - ringR} y={cy - 4} width={ringR * 2 * (progress.value / 100)} height={8} rx={4}
          fill={progress.value >= 100 ? '#4ade80' : 'var(--accent)'} />
      {/if}
    </svg>
  {/if}

  <!-- ── hover detection zone ── invisible area around the pill for hover expand ── -->
  {#if moduleSettings.hover_expand === 'true' && !ui.expanded}
    <div
      class="hover-zone"
      class:charging={showChargingIndicator}
      onmouseenter={handleMouseEnter}
      onmouseleave={handleMouseLeave}
      aria-hidden="true"
    ></div>
  {/if}

  <div
    class="island"
    class:open={ui.expanded}
    class:charging={showChargingIndicator && !ui.expanded}
    onmouseenter={handleMouseEnter}
    onmouseleave={handleMouseLeave}
    role="group"
    aria-label="Isle"
  >
    {#if !ui.expanded}
      <button
        class="mini"
        class:has-progress={progress.value >= 0 && progress.label}
        onclick={handlePillClick}
        aria-label="Open Isle"
        in:fade={{ duration: 160, delay: 220 }}
      >
        {#if showMusic}
          <span class="art" style:background-image={media.art ? `url("${media.art}")` : 'none'}></span>
          <span class="eq" aria-hidden="true"><i></i><i></i><i></i></span>
        {:else}
          <span class="clock">{clock}</span>
        {/if}

        {#if !showMusic && weather?.ok && weatherTemp}
          <span class="weather" title={weatherTitle} in:fade={{ duration: 200 }}>
            <Icon name={weatherIcon} size={12} />
            <span class="wtemp">{weatherTemp}</span>
          </span>
        {/if}

        {#if showChargingIndicator}
          <span class="bolt" aria-label="Charging" in:fade={{ duration: 180 }} out:fade={{ duration: 150 }}>⚡</span>
        {:else if progress.battery !== null}
          <span class="battery-dot" style:background={battColour}
            title="Battery {Math.round((progress.battery ?? 0) * 100)}%"></span>
        {/if}

        {#if progress.value >= 0 && progress.label}
          <span class="prog-label" in:fade={{ duration: 140 }} out:fade={{ duration: 100 }}>
            {progress.label} {Math.round(progress.value)}%
          </span>
        {/if}
      </button>
    {/if}
    {#if ui.expanded}
      <div class="content" in:fly={{ y: 10, duration: 260, delay: 140 }} out:fade={{ duration: 90 }}>
        <nav class="tabs" aria-label="Sections" style:--i={index} style:--tab-count={tabs.length}>
          <span class="pill" aria-hidden="true"></span>
          {#each tabs as t}
            <button class="tab" class:active={ui.tab === t.id} onclick={() => handleTabClick(t.id)}
              aria-label={t.label} title="{t.label}" aria-current={ui.tab === t.id}>
              <Icon name={t.icon} />
            </button>
          {/each}
        </nav>
        <section class="body">
          {#key ui.tab}
            <div class="pane">
              {#if ui.tab === 'music'}<Music />
              {:else if ui.tab === 'clips'}<Clips />
              {:else if ui.tab === 'tasks'}<TaskCalendar />
              {:else if ui.tab === 'utility'}<UtilityDrawer />
              {:else if ui.tab === 'workspace'}<Workspace />
              {:else if ui.tab === 'settings'}
                <div class="settings-hint">Opening settings…</div>
              {/if}
            </div>
          {/key}
        </section>
        {#if ui.toast}<div class="toast" class:toast-top={ui.toastPosition === 'top'} in:fade={{ duration: 120 }} out:fade={{ duration: 160 }}>{ui.toast}</div>{/if}
      </div>
    {/if}
  </div>
</main>

<style>
  .stage { position: fixed; inset: 0; display: flex; justify-content: var(--setting-pos, center); align-items: flex-start; padding-top: 6px; }
  :global(body) {
    background: transparent;
    color: #e8e8ea;
    font-family: 'Segoe UI', system-ui, sans-serif;
    font-size: 14px;
    height: 100vh;
    overflow: hidden;
  }
  :global(html) {
    --tab-count: 6;
  }

  /* ── progress ring ─────────────────────────────────────────────────── */
  .progress-ring {
    position: absolute;
    /* centred over the pill: stage uses flex + padding-top:6px, island is 38px tall */
    top: 6px; /* same as stage padding-top */
    left: 50%;
    transform: translateX(-50%);
    pointer-events: none;
    overflow: visible;
    z-index: 1;
    width: auto;
    height: auto;
  }
  .arc {
    transition: stroke-dashoffset .45s cubic-bezier(.4,0,.2,1), stroke .3s ease;
  }
  .progress-ring rect {
    transition: width .45s cubic-bezier(.4,0,.2,1), fill .3s ease;
  }

  /* ── hover detection zone ─────────────────────────────────────────────── */
  .hover-zone {
    position: absolute;
    top: 6px;
    left: 50%;
    transform: translateX(-50%);
    width: calc(var(--setting-pill-w, 200px) + 100px);
    height: calc(38px + 60px);
    pointer-events: auto;
    z-index: 3;
  }
  .hover-zone.charging {
    width: calc(var(--setting-pill-w, 200px) + 116px);
  }

  /* ── island ────────────────────────────────────────────────────────── */
  .island {
    --w: var(--setting-pill-w, 200px); --h: 38px; --r: var(--setting-radius, 19px);
    position: relative; width: var(--w); height: var(--h); border-radius: var(--r);
    background: var(--setting-bg, transparent); overflow: hidden;
    opacity: var(--setting-opacity, 1);
    box-shadow: none;
    border: var(--setting-border, none);
    transition: width .6s var(--spring), height .6s var(--spring), border-radius .6s var(--spring);
    animation: island-in .5s var(--spring) both;
    z-index: 1;
  }
  /* the pill drops in once, when the window first paints */
  @keyframes island-in {
    from { transform: translateY(-12px) scale(.94); }
    to   { transform: none; }
  }
  .island.open     { --w: 500px; --h: 326px; --r: 30px; }
  .island.charging { --w: calc(var(--setting-pill-w, 200px) + 16px); }

  /* ── collapsed ─────────────────────────────────────────────────────── */
  .mini {
    position: absolute; inset: 0; width: 100%;
    display: flex; align-items: center; justify-content: center;
    gap: 0; padding: 0 12px;
  }
  .mini:has(.art) { justify-content: space-between; }
  /* While a download/install is running the pill reads as one line:
     art · label + percent · battery. The equaliser is dropped so the
     text is never drawn over the animation. */
  .mini.has-progress .eq { display: none; }
  .mini.has-progress .clock { flex: 0 0 auto; }
  .mini.has-progress .prog-label {
    position: static; transform: none; margin-left: 6px;
    font-size: 10px; letter-spacing: .02em; opacity: .95;
  }
  .clock { font-variant-numeric: tabular-nums; font-weight: 600; letter-spacing: .02em; flex: 1; text-align: center; }
  .art   { width: 24px; height: 24px; border-radius: 50%; background: var(--row-hi) center/cover; flex-shrink: 0; }

  .eq { display: flex; align-items: flex-end; gap: 3px; height: 14px; flex-shrink: 0; }
  /* The bars settle after three cycles instead of looping forever: an infinite
     transform animation on a transparent always-on-top window keeps WebView2's
     GPU process compositing non-stop (measured ~7.8 % of a core while idle).
     Three cycles is enough to read as "playing", then it holds a static shape. */
  .eq i { width: 3px; height: 14px; border-radius: 2px; background: var(--accent); transform-origin: bottom; animation: eq 0.9s ease-in-out 3; }
  .eq i:nth-child(2) { animation-delay: -.3s; } .eq i:nth-child(3) { animation-delay: -.6s; }
  @keyframes eq { 0%, 100% { transform: scaleY(.25); } 50% { transform: scaleY(1); } }

  /* charging bolt — pure CSS @keyframes, zero JS loop needed.
     The bolt element is added/removed by Svelte reactivity; CSS handles the animation. */
  .bolt {
    font-size: 12px; line-height: 1; flex-shrink: 0; margin-left: 4px;
    /* finite, like the equaliser: no permanent compositor work on the pill */
    animation: pulse-bolt 1.6s ease-in-out 2;
  }
  @keyframes pulse-bolt {
    0%, 100% { opacity: .65; transform: scale(1);    }
    50%       { opacity: 1;   transform: scale(1.18); }
  }

  /* battery dot */
  .battery-dot {
    width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0;
    margin-left: 5px; transition: background .5s ease;
    box-shadow: 0 0 5px currentColor;
  }

  /* weather chip: current conditions, opt-in via Settings → Weather */
  .weather {
    display: flex; align-items: center; gap: 3px; flex-shrink: 0;
    margin-left: 6px; color: var(--ink); opacity: .92;
    font-size: 11px; font-variant-numeric: tabular-nums;
  }
  .weather :global(svg) { color: var(--accent); }
  .wtemp { font-weight: 650; letter-spacing: -.2px; }

  /* progress label (appears below clock text, micro-font) */
  .prog-label {
    position: absolute; bottom: 3px; left: 50%; transform: translateX(-50%);
    font-size: 9px; font-weight: 700; letter-spacing: .05em;
    color: var(--accent); white-space: nowrap; pointer-events: none; opacity: .85;
  }

  /* ── expanded ──────────────────────────────────────────────────────── */
  .content { position: absolute; top: 0; left: 50%; width: 500px; height: 326px; margin-left: -250px; display: flex; flex-direction: column; background: transparent; }
  .tabs { position: relative; display: grid; grid-template-columns: repeat(var(--tab-count, 6), 1fr); margin: 12px 16px 0; padding: 3px; background: var(--row); border-radius: 14px; flex: none; }
  .pill { position: absolute; top: 3px; bottom: 3px; left: 3px; width: calc((100% - 6px) / var(--tab-count, 6)); border-radius: 11px; background: var(--row-hi);
    transform: translateX(calc(var(--i) * 100%)); transition: transform .45s var(--spring); }
  .tab { position: relative; height: 30px; display: grid; place-items: center; color: var(--dim); transition: color .2s; }
  .tab.active, .tab:hover { color: var(--ink); }
  .tab.active { color: var(--accent); }
  .body { flex: 1; min-height: 0; overflow: hidden; }
  /* CSS enter animation rather than an outro: the old pane leaves at once, so
     the new one can never be pushed out of the 278px body by a stale sibling */
  .pane { height: 100%; min-height: 0; animation: pane-in .26s cubic-bezier(.22,.9,.3,1) both; }
  @keyframes pane-in {
    from { opacity: 0; transform: translateY(8px) scale(.995); }
    to   { opacity: 1; transform: none; }
  }
  .settings-hint {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--dim);
    font-size: 13px;
  }
  .toast { position: absolute; left: 50%; bottom: 10px; transform: translateX(-50%); background: #fff; color: #111; font-weight: 600;
    padding: 6px 12px; border-radius: 999px; max-width: 380px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; pointer-events: none; z-index: 2000; }
  .toast.toast-top { bottom: auto; top: 16px; }
</style>
