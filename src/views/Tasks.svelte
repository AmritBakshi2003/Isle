<!-- Tasks: plain local to-do list (table `tasks`). `due` exists in the DB/API but has no UI yet. -->
<script>
  import { call } from '../lib/api.js';
  import Icon from '../lib/Icon.svelte';

  let tasks = $state([]);
  let title = $state('');
  const load = async () => (tasks = await call('task_list'));
  load();

  async function add() {
    if (!title.trim()) return;
    await call('task_add', { title, due: null });
    title = '';
    load();
  }
  const toggle = async (t) => { await call('task_toggle', { id: t.id }); load(); };
  const del = async (t) => { await call('task_delete', { id: t.id }); load(); };
  const clearDone = async () => { await call('task_clear_done'); load(); };
  const doneCount = $derived(tasks.filter((t) => t.done).length);
</script>

<div class="view">
  <div class="top">
    <input class="grow" placeholder="Add a task" bind:value={title} onkeydown={(e) => e.key === 'Enter' && add()} aria-label="New task" />
    <button class="primary" onclick={add}>Add</button>
  </div>
  <div class="scroll">
    {#each tasks as t (t.id)}
      <div class="row" class:done={t.done}>
        <button class="check" class:on={t.done} aria-label={t.done ? 'Mark as not done' : 'Mark as done'} onclick={() => toggle(t)}>
          {#if t.done}<Icon name="check" size={14} />{/if}
        </button>
        <span class="grow clip">{t.title}</span>
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
  .check { width: 20px; height: 20px; border-radius: 50%; border: 1.6px solid var(--dim); display: grid; place-items: center; flex: none; color: #0a0a0a; transition: background .2s, border-color .2s; }
  .check.on { background: var(--accent); border-color: var(--accent); }
  .done .clip { color: var(--dim); text-decoration: line-through; }
  .end { align-self: flex-end; }
</style>
