<!-- Tasks: plain local to-do list (table `tasks`). Supports due dates and reminders with Windows toasts. -->
<script>
  import { call } from '../lib/api.js';
  import Icon from '../lib/Icon.svelte';

  let tasks = $state([]);
  let title = $state('');
  let dueDate = $state('');
  let remindMin = $state(0);
  const load = async () => (tasks = await call('task_list'));
  load();

  async function add() {
    if (!title.trim()) return;
    const due = dueDate ? Math.floor(new Date(dueDate).getTime() / 1000) : null;
    if (remindMin > 0 && due) {
      await call('task_add_with_reminder', { title, due, remindMin });
    } else {
      await call('task_add', { title, due });
    }
    title = '';
    dueDate = '';
    remindMin = 0;
    load();
  }
  const toggle = async (t) => { await call('task_toggle', { id: t.id }); load(); };
  const del = async (t) => { await call('task_delete', { id: t.id }); load(); };
  const cancelReminder = async (t) => { await call('task_cancel_reminder', { id: t.id }); load(); };
  const clearDone = async () => { await call('task_clear_done'); load(); };
  const doneCount = $derived(tasks.filter((t) => t.done).length);
  
  function formatDue(ts) {
    if (!ts) return '';
    const d = new Date(ts * 1000);
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  }
  
  function isOverdue(ts) {
    if (!ts) return false;
    return ts * 1000 < Date.now();
  }
</script>

<div class="view">
  <div class="top">
    <input class="grow" placeholder="Add a task" bind:value={title} onkeydown={(e) => e.key === 'Enter' && add()} aria-label="New task" />
  </div>
  <div class="extra">
    <input type="datetime-local" class="date-input" bind:value={dueDate} aria-label="Due date" />
    <input type="number" class="min-input" placeholder="Remind (min)" bind:value={remindMin} min="0" aria-label="Reminder minutes" />
    <button class="primary" onclick={add}>Add</button>
  </div>
  <div class="scroll">
    {#each tasks as t (t.id)}
      <div class="row" class:done={t.done} class:overdue={isOverdue(t.due)}>
        <button class="check" class:on={t.done} aria-label={t.done ? 'Mark as not done' : 'Mark as done'} onclick={() => toggle(t)}>
          {#if t.done}<Icon name="check" size={14} />{/if}
        </button>
        <div class="grow">
          <span class="clip">{t.title}</span>
          {#if t.due}
            <span class="due" class:overdue={isOverdue(t.due)}>{formatDue(t.due)}</span>
          {/if}
          {#if t.remind_min > 0}
            <span class="reminder">🔔 {t.remind_min}m</span>
          {/if}
        </div>
        {#if t.remind_min > 0}
          <button class="icon-btn hover-only" aria-label="Cancel reminder" onclick={() => cancelReminder(t)}><Icon name="bell-off" size={16} /></button>
        {/if}
        <button class="icon-btn hover-only" aria-label="Delete task" onclick={() => del(t)}><Icon name="trash" size={16} /></button>
      </div>
    {:else}
      <p class="empty">Nothing to do. Type a task above and press Enter.</p>
    {/each}
  </div>
  {#if doneCount}<button class="ghost end" onclick={clearDone}>Clear {doneCount} done</button>{/if}
</div>

<style>
  .top { display: flex; gap: 6px; }
  .extra { display: flex; gap: 6px; margin-top: 6px; }
  .date-input { width: 160px; font-size: 13px; padding: 6px 8px; border: 1px solid var(--dim); border-radius: 8px; background: var(--bg); color: var(--fg); }
  .min-input { width: 90px; font-size: 13px; padding: 6px 8px; border: 1px solid var(--dim); border-radius: 8px; background: var(--bg); color: var(--fg); }
  .check { width: 20px; height: 20px; border-radius: 50%; border: 1.6px solid var(--dim); display: grid; place-items: center; flex: none; color: #0a0a0a; transition: background .2s, border-color .2s; }
  .check.on { background: var(--accent); border-color: var(--accent); }
  .done .clip { color: var(--dim); text-decoration: line-through; }
  .due { font-size: 11px; color: var(--dim); margin-left: 8px; }
  .due.overdue { color: #ff6b6b; }
  .overdue .clip { color: #ff6b6b; }
  .reminder { font-size: 11px; color: var(--accent); margin-left: 4px; }
  .end { align-self: flex-end; }
</style>
