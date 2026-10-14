<!-- Calendar: full month view + agenda for the selected day.
     Events are local only (table `events`); reminders are fired by scheduler.rs as toasts. -->
<script>
  import { call } from '../lib/api.js';
  import { startOfDay, addDays, secs, fmtTs } from '../lib/util.js';
  import Icon from '../lib/Icon.svelte';

  let currentMonth = $state(new Date());
  let selectedDate = $state(startOfDay());
  let events = $state([]);
  let adding = $state(false);
  let title = $state('');
  let time = $state(nextHour());
  let remind = $state(10);

  function nextHour() {
    const d = new Date();
    return `${String((d.getHours() + 1) % 24).padStart(2, '0')}:00`;
  }

  function getMonthDays(date) {
    const year = date.getFullYear();
    const month = date.getMonth();
    const firstDay = new Date(year, month, 1);
    const lastDay = new Date(year, month + 1, 0);
    const startPadding = firstDay.getDay(); // 0 = Sunday
    const totalDays = lastDay.getDate();
    
    const days = [];
    // Add padding days from previous month
    for (let i = 0; i < startPadding; i++) {
      days.push(null);
    }
    // Add actual days of month
    for (let i = 1; i <= totalDays; i++) {
      days.push(new Date(year, month, i));
    }
    return days;
  }

  const monthDays = $derived(getMonthDays(currentMonth));
  const monthName = $derived(currentMonth.toLocaleDateString([], { month: 'long', year: 'numeric' }));

  const load = async () => {
    const firstDay = new Date(currentMonth.getFullYear(), currentMonth.getMonth(), 1);
    const lastDay = new Date(currentMonth.getFullYear(), currentMonth.getMonth() + 1, 0);
    events = await call('event_list', { from: secs(firstDay), to: secs(addDays(lastDay, 1)) });
  };
  load();

  const dayEvents = (date) => {
    if (!date) return [];
    const dayStart = secs(startOfDay(date));
    const dayEnd = secs(addDays(startOfDay(date), 1));
    return events.filter((e) => e.start >= dayStart && e.start < dayEnd);
  };

  const agenda = $derived(dayEvents(selectedDate));
  const selectedDayIndex = $derived(monthDays.findIndex(d => d && d.toDateString() === selectedDate.toDateString()));

  function prevMonth() {
    currentMonth = new Date(currentMonth.getFullYear(), currentMonth.getMonth() - 1, 1);
    load();
  }

  function nextMonth() {
    currentMonth = new Date(currentMonth.getFullYear(), currentMonth.getMonth() + 1, 1);
    load();
  }

  function goToToday() {
    currentMonth = new Date();
    selectedDate = startOfDay();
    load();
  }

  async function add() {
    if (!title.trim()) return;
    const [h, m] = time.split(':').map(Number);
    const d = new Date(selectedDate);
    d.setHours(h, m, 0, 0);
    await call('event_add', { title, start: secs(d), end: null, remindMin: Number(remind) });
    title = ''; adding = false; load();
  }
  const del = async (e) => { await call('event_delete', { id: e.id }); load(); };
</script>

<div class="view">
  <div class="calendar-layout">
    <div class="calendar-section">
      <div class="calendar-header">
        <button class="icon-btn" aria-label="Previous month" onclick={prevMonth}><Icon name="back" size={14} /></button>
        <strong>{monthName}</strong>
        <button class="icon-btn" aria-label="Next month" onclick={nextMonth}><Icon name="forward" size={14} /></button>
        <button class="ghost" onclick={goToToday}>Today</button>
      </div>

      <div class="weekdays">
        {#each ['S','M','T','W','T','F','S'] as day}
          <span class="weekday">{day}</span>
        {/each}
      </div>

      <div class="month-grid">
        {#each monthDays as date, i}
          {#if date}
            <button 
              class="day-cell" 
              class:sel={date.toDateString() === selectedDate.toDateString()}
              class:today={date.toDateString() === new Date().toDateString()}
              onclick={() => (selectedDate = date)}
              aria-label={date.toLocaleDateString()}
              aria-pressed={date.toDateString() === selectedDate.toDateString()}
            >
              <span class="day-num">{date.getDate()}</span>
              {#if dayEvents(date).length > 0}
                <div class="event-dots">
                  {#each Array(Math.min(dayEvents(date).length, 3)) as _}
                    <span class="dot"></span>
                  {/each}
                </div>
              {/if}
            </button>
          {:else}
            <div class="day-cell empty"></div>
          {/if}
        {/each}
      </div>
    </div>

    <div class="agenda-section">
      <div class="agenda-header">
        <strong>{selectedDate.toLocaleDateString([], { weekday: 'short', month: 'short', day: 'numeric' })}</strong>
      </div>
      
      {#if adding}
        <div class="form">
          <input placeholder="Event title" bind:value={title} onkeydown={(e) => e.key === 'Enter' && add()} aria-label="Event title" />
          <div class="line">
            <input type="time" bind:value={time} aria-label="Start time" />
            <select bind:value={remind} aria-label="Reminder">
              <option value={0}>No reminder</option><option value={5}>5 min</option><option value={10}>10 min</option><option value={30}>30 min</option><option value={60}>1 h</option>
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
              <button class="icon-btn hover-only" aria-label="Delete event" onclick={() => del(e)}><Icon name="trash" size={14} /></button>
            </div>
          {:else}
            <p class="empty">No events</p>
          {/each}
        </div>
        <button class="ghost end" onclick={() => (adding = true)}><Icon name="plus" size={12} /> New event</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .calendar-layout { display: flex; gap: 12px; height: 100%; }
  
  .calendar-section { flex: none; display: flex; flex-direction: column; }
  .calendar-header { display: flex; align-items: center; gap: 6px; margin-bottom: 4px; }
  .calendar-header strong { font-size: 13px; }
  .weekdays { display: grid; grid-template-columns: repeat(7, 1fr); gap: 1px; margin-bottom: 2px; }
  .weekday { text-align: center; font-size: 9px; color: var(--dim); font-weight: 600; }
  
  .month-grid { display: grid; grid-template-columns: repeat(7, 1fr); gap: 1px; }
  .day-cell { 
    aspect-ratio: 1; 
    display: flex; 
    flex-direction: column; 
    align-items: center; 
    justify-content: center; 
    border-radius: 4px; 
    transition: background .2s; 
    position: relative;
    min-height: 24px;
    max-height: 24px;
  }
  .day-cell:not(.empty) { cursor: pointer; }
  .day-cell:hover:not(.empty) { background: var(--row); }
  .day-cell.sel { background: var(--accent); color: #0a0a0a; }
  .day-cell.today { font-weight: 700; }
  .day-cell.empty { background: transparent; }
  
  .day-num { font-size: 11px; font-variant-numeric: tabular-nums; }
  .event-dots { display: flex; gap: 1px; margin-top: 1px; }
  .dot { width: 2px; height: 2px; border-radius: 50%; background: currentColor; opacity: 0.7; }
  
  .agenda-section { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .agenda-header { margin-bottom: 6px; font-size: 12px; flex-shrink: 0; }
  
  .time { font-variant-numeric: tabular-nums; width: 48px; font-size: 11px; }
  .form { display: flex; flex-direction: column; gap: 6px; }
  .line { display: flex; gap: 4px; align-items: center; }
  .line select { flex: 1; font-size: 11px; }
  .end { align-self: flex-end; display: inline-flex; gap: 4px; }
</style>
