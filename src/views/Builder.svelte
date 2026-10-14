<!--
  Builder: create/edit a Mode or a Routine. Both are "a name + a list of actions";
  a routine also has a trigger (manual, or a time of day on chosen weekdays).
  Action JSON schema lives in src-tauri/src/actions.rs. To expose a NEW action
  type to users: add a match arm there AND an entry to ACTION_TYPES here.
-->
<script>
  import { untrack } from 'svelte';
  import { call } from '../lib/api.js';
  import { toast } from '../lib/state.svelte.js';
  import Icon from '../lib/Icon.svelte';

  let { kind, item = null, onsave, oncancel } = $props();
  // The form edits a private copy; it intentionally reads `item` once, at mount.
  const init = untrack(() => item);

  const ACTION_TYPES = [
    { type: 'open', label: 'Open app, file or link', field: 'target', hint: 'C:\\Program Files\\App\\app.exe or https://…' },
    { type: 'wallpaper_next', label: 'Next wallpaper' },
    { type: 'notify', label: 'Show a notification', field: 'title', hint: 'Notification text' },
    { type: 'media', label: 'Play / pause music', fixed: { cmd: 'play_pause' } },
  ];
  const DAYS = ['S', 'M', 'T', 'W', 'T', 'F', 'S']; // index = JS getDay(), 0 = Sunday
  const label = (a) => {
    const t = ACTION_TYPES.find((x) => x.type === a.type);
    const v = a[t?.field] ?? '';
    return `${t?.label ?? a.type}${v ? `: ${v}` : ''}`;
  };

  let name = $state(init?.name ?? '');
  let icon = $state(init?.icon ?? '⚡');
  let actions = $state(structuredClone($state.snapshot(init?.actions ?? [])));
  let when = $state(init?.trigger?.kind === 'time' ? 'time' : 'manual');
  let at = $state(init?.trigger?.at ?? '09:00');
  let days = $state(init?.trigger?.days ?? [1, 2, 3, 4, 5]);
  let newType = $state('open');
  let newValue = $state('');
  const spec = $derived(ACTION_TYPES.find((t) => t.type === newType));

  function addAction() {
    const a = { type: newType, ...(spec.fixed ?? {}) };
    if (spec.field) {
      if (!newValue.trim()) return toast('Enter a value first');
      a[spec.field] = newValue.trim();
      if (newType === 'notify') a.body = '';
    }
    actions.push(a);
    newValue = '';
  }
  const toggleDay = (i) => (days = days.includes(i) ? days.filter((d) => d !== i) : [...days, i]);

  async function save() {
    if (!name.trim()) return toast('Give it a name');
    const acts = $state.snapshot(actions);
    try {
      if (kind === 'mode') await call('mode_save', { id: item?.id ?? null, name: name.trim(), icon, actions: acts });
      else {
        const trigger = when === 'time' ? { kind: 'time', at, days: $state.snapshot(days) } : { kind: 'manual' };
        await call('routine_save', { id: item?.id ?? null, name: name.trim(), trigger, actions: acts });
      }
      onsave();
    } catch (e) { toast(e); }
  }
</script>

<div class="view">
  <div class="head">
    <button class="icon-btn" aria-label="Back" onclick={oncancel}><Icon name="back" /></button>
    <strong>{item ? 'Edit' : 'New'} {kind}</strong>
    <button class="primary push" onclick={save}>Save</button>
  </div>
  <div class="scroll">
    <div class="line">
      {#if kind === 'mode'}<input class="emoji" bind:value={icon} maxlength="2" aria-label="Icon (emoji)" />{/if}
      <input class="grow" placeholder="Name" bind:value={name} aria-label="Name" />
    </div>

    {#if kind === 'routine'}
      <div class="line">
        <select bind:value={when} aria-label="Trigger">
          <option value="manual">Run manually</option><option value="time">At a time</option>
        </select>
        {#if when === 'time'}<input type="time" bind:value={at} aria-label="Time" />{/if}
      </div>
      {#if when === 'time'}
        <div class="line" role="group" aria-label="Days">
          {#each DAYS as d, i}
            <button class="dow" class:on={days.includes(i)} onclick={() => toggleDay(i)} aria-pressed={days.includes(i)}>{d}</button>
          {/each}
        </div>
      {/if}
    {/if}

    <h3>Actions</h3>
    {#each actions as a, i}
      <div class="row"><span class="grow clip">{label(a)}</span>
        <button class="icon-btn" aria-label="Remove action" onclick={() => actions.splice(i, 1)}><Icon name="trash" size={15} /></button></div>
    {:else}
      <p class="dim">No actions yet. Add what should happen.</p>
    {/each}
    <div class="line">
      <select bind:value={newType} aria-label="Action type">{#each ACTION_TYPES as t}<option value={t.type}>{t.label}</option>{/each}</select>
      {#if spec.field}<input class="grow" placeholder={spec.hint} bind:value={newValue} onkeydown={(e) => e.key === 'Enter' && addAction()} aria-label="Action value" />{/if}
      <button class="primary" onclick={addAction}>Add</button>
    </div>
  </div>
</div>

<style>
  .head { display: flex; align-items: center; gap: 8px; }
  .push { margin-left: auto; }
  .line { display: flex; gap: 6px; align-items: center; }
  .emoji { width: 48px; text-align: center; }
  .dow { width: 32px; height: 32px; border-radius: 50%; background: var(--row); color: var(--dim); }
  .dow.on { background: var(--accent); color: #0a0a0a; font-weight: 600; }
</style>
