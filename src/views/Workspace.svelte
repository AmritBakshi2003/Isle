<!--
  Workspace.svelte: the system & productivity dashboard.

  Layout (500 x 278 expanded body):
    top row   Network | Today          — 50/50, deliberately compact
    bottom    Running apps             — CPU / memory / GPU as % of this PC,
                                        with the keep-on-top macro per row

  Everything is event-driven:
    network.rs  -> `workspace://network`  1 Hz
    wellbeing.rs-> `wellbeing://changed`  on every app switch
    procs.rs    -> `workspace://apps`     1 Hz

  DUTY CYCLE (invariant #1): both samplers are parked until this component
  mounts and are parked again on unmount; the backend also skips ticks while
  the island window is hidden. There is no setInterval in this file.
-->
<script>
  import { onDestroy, onMount } from 'svelte';
  import { call, on } from '../lib/api.js';
  import { toast } from '../lib/state.svelte.js';
  import Icon from '../lib/Icon.svelte';

  /* ---------------------------------------------------------------- network */
  const MAX_POINTS = 40;
  let net = $state(null);
  let history = $state([]);
  let netTarget = $state('');

  /* -------------------------------------------------------------- wellbeing */
  let apps = $state([]);
  let logging = $state(true);

  /* ------------------------------------------------------------- resources */
  let procs = $state([]);
  let gpuAvailable = $state(true);
  let cores = $state(0);
  let ramMb = $state(0);

  let unmounted = false;

  /* ------------------------------------------------------------ formatting */
  const fmtBps = (bps) => {
    const v = bps || 0;
    if (v < 1000) return '0 b/s';
    if (v < 1_000_000) return `${(v / 1000).toFixed(0)} kb/s`;
    if (v < 10_000_000) return `${(v / 1_000_000).toFixed(1)} Mb/s`;
    if (v < 1_000_000_000) return `${(v / 1_000_000).toFixed(0)} Mb/s`;
    return `${(v / 1_000_000_000).toFixed(1)} Gb/s`;
  };
  const fmtLink = (bps) => (bps ? fmtBps(bps).replace('/s', '') : '—');
  const fmtDur = (s) => {
    const v = Math.max(0, Math.round(s || 0));
    if (v < 60) return `${v}s`;
    if (v < 3600) return `${Math.floor(v / 60)}m`;
    return `${Math.floor(v / 3600)}h ${Math.floor((v % 3600) / 60)}m`;
  };
  /** One decimal below 10 %, whole numbers above — keeps the column narrow. */
  const fmtPct = (v) => {
    const n = Math.max(0, Math.min(100, Number(v) || 0));
    return n >= 9.95 ? `${Math.round(n)}%` : `${n.toFixed(1)}%`;
  };
  const loadClass = (v) => (v >= 75 ? 'hi' : v >= 40 ? 'mid' : '');

  /* -------------------------------------------------------------- sparkline */
  const spark = $derived.by(() => {
    const h = history;
    if (h.length < 2) return '';
    const peak = Math.max(...h, 1);
    const step = 100 / (h.length - 1);
    return h
      .map((v, i) => `${i ? 'L' : 'M'}${(i * step).toFixed(1)},${(16 - (v / peak) * 14).toFixed(1)}`)
      .join(' ');
  });

  /* ---------------------------------------------------------------- helpers */
  const totalSeconds = $derived(apps.reduce((n, a) => n + (a.seconds || 0), 0));
  const share = (a) => `${Math.max(2, Math.round(((a.seconds || 0) / (totalSeconds || 1)) * 100))}%`;
  // the card is only ~110 px tall, so show the top three
  const topApps = $derived(apps.slice(0, 3));

  function applySample(s) {
    if (!s) return;
    net = s;
    history = [...history.slice(-(MAX_POINTS - 1)), s.down_bps || 0];
  }

  function applyProcs(s) {
    if (!s) return;
    procs = s.rows ?? [];
    gpuAvailable = s.gpu_available !== false;
    cores = s.cores ?? 0;
    ramMb = s.mem_total_mb ?? 0;
  }

  async function loadNetSettings() {
    try {
      const c = await call('net_settings_get');
      netTarget = c.public ? c.host : c.gateway || 'gateway';
    } catch (e) {
      console.warn('net settings:', e);
    }
  }

  let wbTimer;
  async function loadWellbeing() {
    clearTimeout(wbTimer);
    wbTimer = setTimeout(async () => {
      if (unmounted) return;
      try {
        const [rows, enabled] = await Promise.all([
          call('wellbeing_today'),
          call('wellbeing_enabled_get'),
        ]);
        if (unmounted) return;
        apps = rows ?? [];
        logging = enabled !== false;
      } catch (e) {
        console.warn('wellbeing:', e);
      }
    }, 250);
  }

  async function toggleFloat(row) {
    try {
      if (row.topmost) await call('topmost_release', { hwnd: row.hwnd });
      else await call('topmost_float', { hwnd: row.hwnd });
      toast(row.topmost ? `Released “${row.app}”` : `Keeping “${row.app}” on top`);
      // reflect it immediately; the next tick confirms
      procs = procs.map((p) => (p.hwnd === row.hwnd ? { ...p, topmost: !row.topmost } : p));
    } catch (e) {
      toast(String(e));
    }
  }

  async function releaseAll() {
    try {
      const n = await call('topmost_release_all');
      if (!n) return toast('Nothing was floating');
      toast(`Released ${n} window${n === 1 ? '' : 's'}`);
      procs = procs.map((p) => ({ ...p, topmost: false }));
    } catch (e) {
      toast(String(e));
    }
  }

  async function enableLogging() {
    try {
      await call('wellbeing_enabled_set', { enabled: true });
      await loadWellbeing();
    } catch (e) {
      toast(String(e));
    }
  }

  /* ------------------------------------------------------------- lifecycle */
  let offNet, offWb, offApps;

  onMount(() => {
    offNet = on('workspace://network', applySample);
    offWb = on('wellbeing://changed', loadWellbeing);
    offApps = on('workspace://apps', applyProcs);

    call('net_snapshot').then(applySample).catch(() => {});
    call('procs_snapshot').then(applyProcs).catch(() => {});
    call('net_sampler_set', { enabled: true }).catch((e) => console.warn('net sampler:', e));
    call('procs_set', { enabled: true }).catch((e) => console.warn('procs sampler:', e));
    loadNetSettings();
    loadWellbeing();
  });

  onDestroy(() => {
    unmounted = true;
    clearTimeout(wbTimer);
    offNet?.();
    offWb?.();
    offApps?.();
    // park both samplers: nothing ticks while this tab is closed
    call('net_sampler_set', { enabled: false }).catch(() => {});
    call('procs_set', { enabled: false }).catch(() => {});
  });
</script>

<div class="view">
  <div class="top">
    <!-- ── network (compact) ─────────────────────────────────────────── -->
    <section class="card net">
      <header>
        <Icon name="performance" size={12} />
        <h2>Network</h2>
        <span class="grow"></span>
        {#if net?.iface}<span class="chip">{net.iface}</span>{/if}
      </header>

      <div class="rates">
        <span class="rate"><i>↓</i>{fmtBps(net?.down_bps)}</span>
        <span class="rate"><i>↑</i>{fmtBps(net?.up_bps)}</span>
      </div>

      <div class="sub" title={netTarget ? `ping ${netTarget}` : 'latency probe off'}>
        <span>{net?.latency_ms != null ? `${net.latency_ms} ms` : '— ms'}</span>
        <span class="dot">·</span>
        <span>{fmtLink(net?.link_down_bps)} link</span>
      </div>

      <svg class="spark" viewBox="0 0 100 16" preserveAspectRatio="none" aria-hidden="true">
        {#if spark}
          <path class="fill" d="{spark} L100,16 L0,16 Z" />
          <path class="line" d={spark} />
        {/if}
      </svg>
    </section>

    <!-- ── digital wellbeing ─────────────────────────────────────────── -->
    <section class="card wb">
      <header>
        <Icon name="progress" size={12} />
        <h2>Today</h2>
        <span class="grow"></span>
        {#if totalSeconds}<span class="tiny">{fmtDur(totalSeconds)}</span>{/if}
      </header>

      {#if topApps.length}
        <ul class="bars">
          {#each topApps as a (a.app)}
            <li>
              <span class="app" title={a.title || a.app}>{a.app}</span>
              <span class="bar"><i style:width={share(a)}></i></span>
              <span class="when">{fmtDur(a.seconds)}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="note">
          {#if logging}
            Nothing logged yet — it fills in as you switch apps.
          {:else}
            Logging is off. <button class="link" onclick={enableLogging}>Turn on</button>
          {/if}
        </p>
      {/if}
    </section>
  </div>

  <!-- ── running apps + window macro ─────────────────────────────────── -->
  <section class="card apps">
    <header>
      <Icon name="pin" size={12} />
      <h2>Running apps</h2>
      <span class="tiny meta">
        {cores ? `${cores} cores` : ''}{cores && ramMb ? ' · ' : ''}{ramMb ? `${(ramMb / 1024).toFixed(0)} GB` : ''}
      </span>
      <span class="grow"></span>
      <button class="ghost" onclick={releaseAll}>Release all</button>
    </header>

    {#if procs.length}
      <div class="grid head" class:no-gpu={!gpuAvailable}>
        <span>app</span>
        <span>cpu</span>
        <span>ram</span>
        {#if gpuAvailable}<span>gpu</span>{/if}
        <span></span>
      </div>
      <ul class="list">
        {#each procs as p (p.pid)}
          <li>
            <button
              class="grid row" class:no-gpu={!gpuAvailable}
              onclick={() => toggleFloat(p)}
              title="{p.title || p.app} — click to {p.topmost ? 'release' : 'keep on top'}"
              aria-pressed={p.topmost}
            >
              <span class="name" class:floating={p.topmost}>
                {#if p.topmost}<i class="pin">•</i>{/if}{p.app}
              </span>
              <span class="metric">
                <b class={loadClass(p.cpu_pct)}>{fmtPct(p.cpu_pct)}</b>
                <em class="track"><i class={loadClass(p.cpu_pct)} style:width="{Math.min(100, p.cpu_pct)}%"></i></em>
              </span>
              <span class="metric">
                <b class={loadClass(p.mem_pct)}>{fmtPct(p.mem_pct)}</b>
                <em class="track"><i class={loadClass(p.mem_pct)} style:width="{Math.min(100, p.mem_pct)}%"></i></em>
              </span>
              {#if gpuAvailable}
                <span class="metric">
                  <b class={loadClass(p.gpu_pct)}>{fmtPct(p.gpu_pct)}</b>
                  <em class="track"><i class={loadClass(p.gpu_pct)} style:width="{Math.min(100, p.gpu_pct)}%"></i></em>
                </span>
              {/if}
              <span class="action" class:on={p.topmost}>
                <Icon name="pin" size={12} />
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="note">No other windows are open — start an app and it shows up here.</p>
    {/if}
  </section>
</div>

<style>
  /* `.view` / `.ghost` / tokens come from app.css. */
  .top {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    flex: none;
  }
  .card {
    background: var(--row);
    border-radius: 14px;
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
    min-height: 0;
  }
  header { display: flex; align-items: center; gap: 5px; color: var(--dim); line-height: 1; }
  header h2 {
    margin: 0;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--dim);
    line-height: 1;
  }
  .chip {
    background: var(--row-hi);
    border-radius: 999px;
    padding: 1px 6px;
    font-size: 9.5px;
    color: var(--ink);
    max-width: 88px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tiny { font-size: 10px; }
  .grow { flex: 1; }

  /* -- network -- */
  .net { height: 104px; }
  .rates { display: flex; gap: 10px; line-height: 1; }
  .rate {
    display: flex;
    align-items: baseline;
    gap: 3px;
    font-size: 14px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.02em;
    white-space: nowrap;
  }
  .rate i { color: var(--dim); font-size: 10px; font-style: normal; }
  .sub {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    color: var(--dim);
    line-height: 1;
    white-space: nowrap;
  }
  .dot { opacity: 0.5; }
  .spark { width: 100%; height: 16px; display: block; margin-top: auto; }
  .spark .line { fill: none; stroke: var(--accent); stroke-width: 1.2; vector-effect: non-scaling-stroke; }
  .spark .fill { fill: var(--accent); opacity: 0.16; stroke: none; }

  /* -- wellbeing -- */
  .wb { height: 104px; }
  .bars { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
  .bars li { display: flex; align-items: center; gap: 6px; font-size: 10.5px; line-height: 12px; min-width: 0; }
  .app { width: 62px; flex: none; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bar { flex: 1; height: 4px; border-radius: 2px; background: var(--row-hi); overflow: hidden; min-width: 0; }
  .bar i { display: block; height: 100%; border-radius: 2px; background: var(--accent); opacity: 0.85; }
  .when { width: 40px; flex: none; text-align: right; color: var(--dim); font-variant-numeric: tabular-nums; }

  /* -- running apps -- */
  .apps { flex: 1; min-height: 0; }
  .apps header .ghost { font-size: 10px; padding: 2px 6px; }
  .meta { color: var(--dim); opacity: 0.8; white-space: nowrap; }

  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 74px 74px 74px 20px;
    align-items: center;
    gap: 8px;
  }
  .grid.no-gpu { grid-template-columns: minmax(0, 1fr) 74px 74px 20px; }
  .grid.head {
    font-size: 9px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--dim);
    opacity: 0.75;
    padding: 0 4px;
  }
  .grid.head span:not(:first-child) { text-align: right; }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
    overflow-y: auto;
    min-height: 0;
  }
  .row {
    width: 100%;
    text-align: left;
    padding: 2px 4px;
    border-radius: 8px;
    background: transparent;
    transition: background 0.15s;
  }
  .row:hover { background: var(--row-hi); }
  .name {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .name.floating { color: var(--accent); }
  .pin { font-style: normal; margin-right: 3px; }

  .metric { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; min-width: 0; }
  .metric b {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--ink);
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }
  .metric b.mid { color: #f0c674; }
  .metric b.hi { color: #f7706a; }
  .track {
    display: block;
    width: 100%;
    height: 2px;
    border-radius: 1px;
    background: var(--row-hi);
    overflow: hidden;
  }
  .track i { display: block; height: 100%; background: var(--accent); opacity: 0.9; }
  .track i.mid { background: #f0c674; }
  .track i.hi { background: #f7706a; }

  .action { display: grid; place-items: center; color: var(--dim); opacity: 0.45; }
  .action.on { color: var(--accent); opacity: 1; }
  .row:hover .action { opacity: 1; }

  .link {
    background: none;
    padding: 0;
    color: var(--accent);
    font-size: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .note { margin: 4px 0 0; font-size: 10.5px; color: var(--dim); line-height: 1.45; }
</style>
