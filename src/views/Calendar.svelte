<!-- Calendar: a 7-day strip starting today + agenda for the selected day.
     Events are local only (table `events`); reminders are fired by scheduler.rs as toasts. -->
<script>
  import { call } from '../lib/api.js';
  import { startOfDay, addDays, secs, fmtTs } from '../lib/util.js';
  import Icon from '../lib/Icon.svelte';

  const days = Array.from({ length: 7 }, (_, i) => addDays(startOfDay(), i));
  let sel = $state(0);
  let events = $state([]);
  let adding = $state(false);
  let title = $state('');
  let time = $state(nextHour());
  let remind = $state(10);

  function nextHour() {
    const d = new Date();
    return `${String((d.getHours() + 1) % 24).padStart(2, '0')}:00`;
  }
  const load = async () => (events = await call('event_list', { from: secs(days[0]), to: secs(addDays(days[6], 1)) }));
  load();

  const dayOf = (i) => events.filter((e) => e.start >= secs(days[i]) && e.start < secs(addDays(days[i], 1)));
  const agenda = $derived(dayOf(sel));

  async function add() {
    if (!title.trim()) return;
    const [h, m] = time.split(':').map(Number);
    const d = new Date(days[sel]);
    d.setHours(h, m, 0, 0);
    await call('event_add', { title, start: secs(d), end: null, remindMin: Number(remind) });
    title = ''; adding = false; load();
  }
  const del = async (e) => { await call('event_delete', { id: e.id }); load(); };
</script>

<div class="view">
  <div class="strip" role="tablist" aria-label="Days">
    {#each days as d, i}
      <button class="day" class:sel={sel === i} role="tab" aria-selected={sel === i} onclick={() => (sel = i)}>
        <span class="dim">{d.toLocaleDateString([], { weekday: 'short' })}</span>
        <strong>{d.getDate()}</strong>
        <i class:dot={dayOf(i).length > 0}></i>
      </button>
    {/each}
  </div>

  {#if adding}
    <div class="form">
      <input placeholder="Event title" bind:value={title} onkeydown={(e) => e.key === 'Enter' && add()} aria-label="Event title" />
      <div class="line">
        <input type="time" bind:value={time} aria-label="Start time" />
        <select bind:value={remind} aria-label="Reminder">
          <option value={0}>No reminder</option><option value={5}>5 min before</option>
          <option value={10}>10 min before</option><option value={30}>30 min before</option><option value={60}>1 h before</option>
        </select>
        <button class="primary" onclick={add}>Save</button>
        <button class="ghost" onclick={() => (adding = false)}>Cancel</button>
      </div>
    </div>
  {:else}
    <div class="scroll">
      {#each agenda as e (e.id)}
        <div class="row">
          <span class="time dim">{fmtTs(e.start)}</span>
          <span class="grow clip">{e.title}</span>
          <button class="icon-btn hover-only" aria-label="Delete event" onclick={() => del(e)}><Icon name="trash" size={16} /></button>
        </div>
      {:else}
        <p class="empty">No events this day.</p>
      {/each}
    </div>
    <button class="ghost end" onclick={() => (adding = true)}><Icon name="plus" size={14} /> New event</button>
  {/if}
</div>

<style>
  .strip { display: grid; grid-template-columns: repeat(7, 1fr); gap: 4px; flex: none; }
  .day { display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 6px 0 5px; border-radius: 12px; transition: background .2s; }
  .day:hover { background: var(--row); }
  .day.sel { background: var(--accent); color: #0a0a0a; }
  .day.sel .dim { color: #0a0a0acc; }
  .day strong { font-size: 15px; font-variant-numeric: tabular-nums; }
  .day i { width: 4px; height: 4px; border-radius: 50%; background: transparent; }
  .day i.dot { background: currentColor; }
  .time { font-variant-numeric: tabular-nums; width: 56px; }
  .form { display: flex; flex-direction: column; gap: 8px; margin-top: 6px; }
  .line { display: flex; gap: 6px; align-items: center; }
  .line select { flex: 1; }
  .end { align-self: flex-end; display: inline-flex; align-items: center; gap: 4px; }
</style>
