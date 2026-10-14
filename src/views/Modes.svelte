<!--
  Modes tab = Modes (one-tap presets) + Routines (automations) + Wallpaper + Startup.
  A mode runs its actions when tapped; the last one is remembered as `active_mode`.
  A routine runs on a schedule (scheduler.rs) or via its play button.
  Builder.svelte edits both. Settings are plain key/value rows (see setting_set).
-->
<script>
  import { call } from '../lib/api.js';
  import { toast } from '../lib/state.svelte.js';
  import Icon from '../lib/Icon.svelte';
  import Builder from './Builder.svelte';

  let modes = $state([]);
  let routines = $state([]);
  let active = $state('');
  let editing = $state(null); // { kind: 'mode' | 'routine', item }
  let dir = $state('');
  let interval = $state('0');
  let autostart = $state(false);

  async function load() {
    [modes, routines] = await Promise.all([call('mode_list'), call('routine_list')]);
    active = (await call('setting_get', { key: 'active_mode' })) ?? '';
    dir = (await call('setting_get', { key: 'wallpaper_dir' })) ?? '';
    interval = (await call('setting_get', { key: 'wallpaper_interval' })) ?? '0';
    autostart = await call('autostart_get');
  }
  load();

  const fail = (e) => toast(e);
  async function activate(m) {
    const isActive = String(m.id) === active;
    try {
      if (isActive) {
        // Deactivate: clear active_mode setting
        await call('setting_set', { key: 'active_mode', value: '' });
        active = '';
        toast(`${m.name} off`);
      } else {
        // Activate: run actions and set active_mode
        await call('mode_activate', { id: m.id });
        active = String(m.id);
        toast(`${m.name} on`);
      }
    } catch (e) { fail(e); }
  }
  const run = (r) => call('routine_run', { id: r.id }).then(() => toast(`${r.name} ran`)).catch(fail);
  const toggle = async (r) => { await call('routine_toggle', { id: r.id, enabled: !r.enabled }); load(); };
  async function remove() {
    const { kind, item } = editing;
    await call(kind === 'mode' ? 'mode_delete' : 'routine_delete', { id: item.id });
    editing = null; load();
  }
  const saveDir = () => call('setting_set', { key: 'wallpaper_dir', value: dir.trim() });
  const saveInterval = () => call('setting_set', { key: 'wallpaper_interval', value: String(Math.max(0, Number(interval) || 0)) });
  const nextWall = () => call('wallpaper_next').then(() => toast('Wallpaper changed')).catch(fail);
  const setAutostart = () => call('autostart_set', { enabled: autostart }).catch((e) => { autostart = !autostart; fail(e); });
  const when = (r) => (r.trigger?.kind === 'time' ? `${r.trigger.at}${r.trigger.days?.length === 5 && !r.trigger.days.includes(0) && !r.trigger.days.includes(6) ? ' weekdays' : ''}` : 'manual');
</script>

{#if editing}
  <Builder kind={editing.kind} item={editing.item} oncancel={() => (editing = null)} onsave={() => { editing = null; load(); }} />
  {#if editing.item}<button class="ghost del" onclick={remove}>Delete {editing.kind}</button>{/if}
{:else}
  <div class="view">
    <div class="scroll">
      <h3>Modes</h3>
      <div class="chips">
        {#each modes as m (m.id)}
          <div class="chip" class:on={String(m.id) === active}>
            <button class="go" onclick={() => activate(m)}><span class="em">{m.icon}</span>{m.name}</button>
            <button class="icon-btn hover-only" aria-label="Edit {m.name}" onclick={() => (editing = { kind: 'mode', item: m })}><Icon name="edit" size={14} /></button>
          </div>
        {/each}
        <button class="chip add" aria-label="New mode" onclick={() => (editing = { kind: 'mode', item: null })}><Icon name="plus" size={16} /></button>
      </div>

      <h3>Routines</h3>
      {#each routines as r (r.id)}
        <div class="row">
          <button class="icon-btn" aria-label="Run {r.name}" onclick={() => run(r)}><Icon name="play2" size={15} /></button>
          <button class="grow name" onclick={() => (editing = { kind: 'routine', item: r })}>
            <span class="clip">{r.name}</span><span class="dim small">{when(r)}</span>
          </button>
          <label class="switch" title={r.enabled ? 'On' : 'Off'}>
            <input type="checkbox" checked={r.enabled} onchange={() => toggle(r)} aria-label="Enable {r.name}" /><span></span>
          </label>
        </div>
      {/each}
      <button class="ghost start" onclick={() => (editing = { kind: 'routine', item: null })}><Icon name="plus" size={14} /> New routine</button>

      <h3>Wallpaper</h3>
      <div class="line">
        <input class="grow" placeholder="Folder with jpg or png images" bind:value={dir} onchange={saveDir} aria-label="Wallpaper folder" />
        <button class="primary" onclick={nextWall}>Change now</button>
      </div>
      <div class="line">
        <span class="dim">Rotate every</span>
        <input class="num" type="number" min="0" bind:value={interval} onchange={saveInterval} aria-label="Rotate every N minutes" />
        <span class="dim">min (0 = off)</span>
      </div>

      <h3>Startup</h3>
      <label class="line"><input type="checkbox" bind:checked={autostart} onchange={setAutostart} /> Open Isle when Windows starts</label>
    </div>
  </div>
{/if}

<style>
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { display: inline-flex; align-items: center; background: var(--row); border-radius: 999px; padding-right: 4px; transition: background .2s; }
  .chip:hover { background: var(--row-hi); }
  .chip.on { background: color-mix(in srgb, var(--accent) 28%, transparent); box-shadow: inset 0 0 0 1.5px var(--accent); }
  .go { padding: 7px 8px 7px 12px; display: inline-flex; gap: 6px; align-items: center; }
  .chip:not(:hover):not(:focus-within) .hover-only { width: 0; margin: 0; }
  .chip .icon-btn { width: 24px; height: 24px; }
  .chip.add { width: 34px; height: 34px; justify-content: center; padding: 0; color: var(--dim); }
  .name { text-align: left; display: flex; flex-direction: column; min-width: 0; }
  .small { font-size: 11px; }
  .start { align-self: flex-start; display: inline-flex; gap: 4px; align-items: center; }
  .line { display: flex; gap: 8px; align-items: center; }
  .num { width: 64px; }
  .del { position: absolute; left: 16px; bottom: 10px; color: #ff8a80; }
  .switch { position: relative; width: 34px; height: 20px; flex: none; }
  .switch input { position: absolute; inset: 0; opacity: 0; margin: 0; cursor: pointer; }
  .switch span { position: absolute; inset: 0; background: var(--row-hi); border-radius: 10px; transition: background .2s; pointer-events: none; }
  .switch span::after { content: ''; position: absolute; top: 3px; left: 3px; width: 14px; height: 14px; border-radius: 50%; background: var(--ink); transition: transform .3s var(--spring); }
  .switch input:checked + span { background: var(--accent); }
  .switch input:checked + span::after { transform: translateX(14px); background: #0a0a0a; }
  .switch input:focus-visible + span { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
