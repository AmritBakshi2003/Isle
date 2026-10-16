<!--
  TaskCalendar.svelte: Unified Tasks & Calendar timeline view
  Displays tasks and events chronologically on a daily timeline.
  Shows Today/Tomorrow toggle pills for quick horizon switching.
-->
<script>
  import { call, on } from '../lib/api.js';
  import { toast } from '../lib/state.svelte.js';
  import Icon from '../lib/Icon.svelte';

  let events = $state([]);
  let tasks = $state([]);
  let horizon = $state('today'); // 'today' or 'tomorrow'
  let selectedDate = $state(new Date());

  // Get start and end of day for the selected horizon
  function getDayRange() {
    const d = new Date(selectedDate);
    if (horizon === 'tomorrow') {
      d.setDate(d.getDate() + 1);
    }
    d.setHours(0, 0, 0, 0);
    const start = Math.floor(d.getTime() / 1000);
    d.setHours(23, 59, 59, 999);
    const end = Math.floor(d.getTime() / 1000);
    return { start, end };
  }

  async function loadData() {
    const { start, end } = getDayRange();
    [events, tasks] = await Promise.all([
      call('event_list', { from: start, to: end }),
      call('task_list'),
    ]);
  }

  // Combine events and tasks into chronological timeline
  let timeline = $derived([...events, ...tasks]
    .filter(item => {
      if (item.starts !== undefined) {
        // Event: check if it falls within the day
        return item.starts >= getDayRange().start && item.starts <= getDayRange().end;
      } else if (item.start_time !== undefined && item.start_time !== null) {
        // Task with time: check if it falls within the day
        return item.start_time >= getDayRange().start && item.start_time <= getDayRange().end;
      }
      // Task without time: include it as flexible
      return !item.done;
    })
    .sort((a, b) => {
      const aTime = a.starts ?? a.start_time ?? 0;
      const bTime = b.starts ?? b.start_time ?? 0;
      return aTime - bTime;
    }));

  // Load data when horizon changes
  $effect(() => {
    loadData();
  });

  // Format time as HH:MM
  function formatTime(ts) {
    if (!ts) return '';
    const d = new Date(ts * 1000);
    return d.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
  }

  // Toggle task completion
  async function toggleTask(task) {
    try {
      await call('task_toggle', { id: task.id });
      await loadData();
    } catch (e) {
      toast(e);
    }
  }

  // Delete event
  async function deleteEvent(event) {
    try {
      await call('event_delete', { id: event.id });
      await loadData();
    } catch (e) {
      toast(e);
    }
  }

  // Delete task
  async function deleteTask(task) {
    try {
      await call('task_delete', { id: task.id });
      await loadData();
    } catch (e) {
      toast(e);
    }
  }

  // Check if item is an event
  function isEvent(item) {
    return item.starts !== undefined;
  }

  // Check if item is a task
  function isTask(item) {
    return item.title !== undefined && !isEvent(item);
  }
</script>

<div class="task-calendar">
  <div class="horizon-tabs">
    <button
      class="horizon-pill"
      class:active={horizon === 'today'}
      onclick={() => horizon = 'today'}
    >
      Today
    </button>
    <button
      class="horizon-pill"
      class:active={horizon === 'tomorrow'}
      onclick={() => horizon = 'tomorrow'}
    >
      Tomorrow
    </button>
  </div>

  <div class="timeline">
    {#if timeline.length === 0}
      <div class="empty">
        <Icon name="calendar" size={32} />
        <p>No {horizon} events or tasks</p>
      </div>
    {:else}
      {#each timeline as item (item.id)}
        <div class="timeline-item" class:event={isEvent(item)} class:task={isTask(item)}>
          <div class="time">
            {formatTime(item.starts ?? item.start_time)}
          </div>
          <div class="content">
            <div class="title">{item.title}</div>
            {#if isEvent(item)}
              <div class="meta">
                {#if item.ends}
                  <span class="dim">until {formatTime(item.ends)}</span>
                {/if}
                {#if item.remind_min > 0}
                  <span class="dim">• {item.remind_min}m reminder</span>
                {/if}
              </div>
            {:else if isTask(item)}
              <div class="meta">
                {#if item.due}
                  <span class="dim">due {formatTime(item.due)}</span>
                {/if}
              </div>
            {/if}
          </div>
          <div class="actions">
            {#if isTask(item)}
              <button
                class="icon-btn"
                class:checked={item.done}
                onclick={() => toggleTask(item)}
                aria-label="Toggle task"
              >
                <Icon name="check" size={16} />
              </button>
            {/if}
            <button
              class="icon-btn delete"
              onclick={() => isEvent(item) ? deleteEvent(item) : deleteTask(item)}
              aria-label="Delete"
            >
              <Icon name="trash" size={14} />
            </button>
          </div>
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .task-calendar {
    padding: 16px;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .horizon-tabs {
    display: flex;
    gap: 8px;
    margin-bottom: 16px;
  }

  .horizon-pill {
    flex: 1;
    padding: 8px 16px;
    background: var(--row);
    border: 1px solid var(--row-hi);
    border-radius: 999px;
    font-size: 13px;
    font-weight: 500;
    color: var(--dim);
    cursor: pointer;
    transition: all 0.2s;
  }

  .horizon-pill:hover {
    background: var(--row-hi);
  }

  .horizon-pill.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #0a0a0a;
  }

  .timeline {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--dim);
  }

  .empty p {
    margin: 0;
    font-size: 14px;
  }

  .timeline-item {
    display: flex;
    gap: 12px;
    padding: 12px;
    background: var(--row);
    border-radius: 10px;
    align-items: center;
    transition: background 0.2s;
  }

  .timeline-item:hover {
    background: var(--row-hi);
  }

  .timeline-item.event {
    border-left: 3px solid var(--accent);
  }

  .timeline-item.task {
    border-left: 3px solid #666;
  }

  .time {
    min-width: 48px;
    font-size: 12px;
    font-weight: 500;
    color: var(--dim);
    text-align: right;
  }

  .content {
    flex: 1;
    min-width: 0;
  }

  .title {
    font-size: 14px;
    font-weight: 500;
    margin-bottom: 4px;
  }

  .meta {
    display: flex;
    gap: 8px;
    font-size: 12px;
  }

  .dim {
    color: var(--dim);
  }

  .actions {
    display: flex;
    gap: 4px;
  }

  .icon-btn {
    width: 28px;
    height: 28px;
    border: none;
    background: transparent;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: var(--dim);
    transition: all 0.2s;
  }

  .icon-btn:hover {
    background: var(--row-hi);
    color: var(--ink);
  }

  .icon-btn.checked {
    color: var(--accent);
  }

  .icon-btn.delete:hover {
    color: #ff8a80;
    background: rgba(255, 138, 128, 0.1);
  }
</style>
