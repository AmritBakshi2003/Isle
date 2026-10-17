<!--
  TaskCalendar.svelte: Unified Tasks & Calendar view
  Left: Month grid with event dots
  Right: Events section (top) and Tasks section (bottom) with Today/Tomorrow pills
-->
<script>
  import { call, on } from '../lib/api.js';
  import { toast } from '../lib/state.svelte.js';
  import Icon from '../lib/Icon.svelte';

  console.log('[TaskCalendar] Component script starting');

  const COMPLETED_TASK_WINDOW_MS = 30 * 60 * 1000; // 30 minutes
  const COLOR_SWATCHES = ['#ef4444', '#f97316', '#f59e0b', '#10b981', '#3b82f6', '#8b5cf6'];

  let events = $state([]);
  let eventDays = $state([]);
  let todayTasks = $state([]);
  let tomorrowTasks = $state([]);
  let selectedDate = $state(new Date().toISOString().split('T')[0]); // YYYY-MM-DD
  let taskHorizon = $state('today'); // 'today' or 'tomorrow'
  let nextChangeTs = $state(0);
  let showEventForm = $state(false);
  let editingEvent = $state(null);
  let editingTask = $state(null);
  let undoStack = $state([]);
  let refreshTimer = $state(null);
  let newTaskInput = $state('');

  // Timezone offset in minutes (cached, doesn't change)
  const tzOffsetMin = -new Date().getTimezoneOffset() / 60;

  // Today's date string (constant for this session)
  const todayStr = new Date().toISOString().split('T')[0];

  // Selected date as Date object (reactive to selectedDate)
  let selectedDateObj = $derived.by(() => {
    try {
      const [year, month, day] = selectedDate.split('-').map(Number);
      return new Date(year, month - 1, day);
    } catch (e) {
      console.error('[TaskCalendar] Error in selectedDateObj:', e);
      return new Date();
    }
  });

  // Year and month from selected date (reactive to selectedDate)
  let currentYear = $derived.by(() => {
    try {
      return selectedDateObj.getFullYear();
    } catch (e) {
      console.error('[TaskCalendar] Error in currentYear:', e);
      return new Date().getFullYear();
    }
  });
  let currentMonth = $derived.by(() => {
    try {
      return selectedDateObj.getMonth() + 1;
    } catch (e) {
      console.error('[TaskCalendar] Error in currentMonth:', e);
      return new Date().getMonth() + 1;
    }
  });

  // Check if selected date is today (reactive to selectedDate)
  let isToday = $derived.by(() => {
    try {
      return selectedDate === todayStr;
    } catch (e) {
      console.error('[TaskCalendar] Error in isToday:', e);
      return false;
    }
  });

  // Get calendar days for current month (reactive to selectedDate and eventDays)
  let calendarDays = $derived.by(() => {
    try {
      const year = currentYear;
      const month = currentMonth;
      const firstDay = new Date(year, month - 1, 1);
      const lastDay = new Date(year, month, 0);
      const startPadding = firstDay.getDay(); // 0 = Sunday
      const totalDays = lastDay.getDate();
      
      const days = [];
      // Add padding days from previous month
      for (let i = 0; i < startPadding; i++) {
        days.push(null);
      }
      // Add actual days of month
      for (let i = 1; i <= totalDays; i++) {
        const date = new Date(year, month - 1, i);
        const dateStr = date.toISOString().split('T')[0];
        const isToday = dateStr === todayStr;
        const hasEvent = eventDays.includes(dateStr);
        days.push({ day: i, date: dateStr, isToday, hasEvent });
      }
      return days;
    } catch (e) {
      console.error('[TaskCalendar] Error in calendarDays:', e);
      return [];
    }
  });

  // Load event days for the month
  async function loadEventDays() {
    console.log('[TaskCalendar] Loading event days for', currentYear, currentMonth);
    try {
      eventDays = await call('month_event_days', {
        year: currentYear,
        month: currentMonth,
        tzOffsetMin: tzOffsetMin
      });
      console.log('[TaskCalendar] Event days loaded:', eventDays);
    } catch (e) {
      console.error('[TaskCalendar] Failed to load event days:', e);
      eventDays = []; // Fallback to empty array
    }
  }

  // Load events for selected date
  async function loadEvents() {
    console.log('[TaskCalendar] Loading events for', selectedDate);
    try {
      const currentNow = Math.floor(Date.now() / 1000);
      events = await call('events_for_day', {
        date: selectedDate,
        nowTs: currentNow,
        tzOffsetMin: tzOffsetMin
      });
      console.log('[TaskCalendar] Events loaded:', events);
    } catch (e) {
      console.error('[TaskCalendar] Failed to load events:', e);
      events = []; // Fallback to empty array
    }
  }

  // Load tasks for today and tomorrow
  async function loadTasks() {
    console.log('[TaskCalendar] Loading tasks');
    try {
      const currentNow = Math.floor(Date.now() / 1000);
      const result = await call('tasks_visible', {
        nowTs: currentNow,
        tzOffsetMin: tzOffsetMin
      });
      todayTasks = result.today || [];
      tomorrowTasks = result.tomorrow || [];
      nextChangeTs = result.next_change_ts || currentNow + 1800; // Rust uses snake_case
      console.log('[TaskCalendar] Tasks loaded:', { today: todayTasks.length, tomorrow: tomorrowTasks.length });
    } catch (e) {
      console.error('[TaskCalendar] Failed to load tasks:', e);
      todayTasks = [];
      tomorrowTasks = [];
      nextChangeTs = Math.floor(Date.now() / 1000) + 1800;
    }
  }

  // Refresh all data
  async function refresh() {
    console.log('[TaskCalendar] Refreshing all data');
    try {
      await Promise.all([loadEventDays(), loadEvents(), loadTasks()]);
      console.log('[TaskCalendar] Refresh complete');
    } catch (e) {
      console.error('[TaskCalendar] Refresh error:', e);
      // Set fallback data so component still renders
      eventDays = [];
      events = [];
      todayTasks = [];
      tomorrowTasks = [];
    }
  }

  // Schedule next refresh at next_change_ts
  function scheduleRefresh() {
    if (refreshTimer) clearTimeout(refreshTimer);
    const currentNow = Math.floor(Date.now() / 1000);
    const delay = Math.max(1000, (nextChangeTs - currentNow) * 1000);
    refreshTimer = setTimeout(() => {
      refresh();
      scheduleRefresh();
    }, delay);
  }

  // Refresh on mount only
  $effect(() => {
    console.log('[TaskCalendar] $effect running, calling refresh');
    try {
      refresh();
      scheduleRefresh();
    } catch (e) {
      console.error('[TaskCalendar] Error in $effect:', e);
    }
    return () => {
      console.log('[TaskCalendar] Cleanup');
      if (refreshTimer) clearTimeout(refreshTimer);
    };
  });

  // Handle window visibility changes
  $effect(() => {
    const handleVisibility = () => {
      if (document.visibilityState === 'visible') {
        refresh();
        scheduleRefresh();
      } else {
        if (refreshTimer) clearTimeout(refreshTimer);
      }
    };
    document.addEventListener('visibilitychange', handleVisibility);
    return () => document.removeEventListener('visibilitychange', handleVisibility);
  });

  // Navigate months
  function prevMonth() {
    const date = new Date(currentYear, currentMonth - 2, 1);
    selectedDate = date.toISOString().split('T')[0];
  }

  function nextMonth() {
    const date = new Date(currentYear, currentMonth, 1);
    selectedDate = date.toISOString().split('T')[0];
  }

  function goToday() {
    selectedDate = todayStr;
  }

  // Select a day
  function selectDay(dateStr) {
    selectedDate = dateStr;
  }

  // Format time as HH:MM
  function formatTime(ts) {
    if (!ts) return '';
    const d = new Date(ts * 1000);
    return d.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
  }

  // Parse date string and time string to unix timestamp
  function parseDateTime(dateStr, timeStr) {
    if (!dateStr) return null;
    const [year, month, day] = dateStr.split('-').map(Number);
    const date = new Date(year, month - 1, day);
    if (timeStr) {
      const [hours, minutes] = timeStr.split(':').map(Number);
      date.setHours(hours, minutes, 0, 0);
    }
    return Math.floor(date.getTime() / 1000);
  }

  // Open event form with selected date prefilled
  function openEventForm() {
    editingEvent = { title: '', date: selectedDate, start: '', end: '', remindMin: 10, color: COLOR_SWATCHES[0] };
    showEventForm = true;
  }

  // Event actions
  async function addEvent() {
    try {
      const start = parseDateTime(editingEvent.date, editingEvent.start);
      const end = editingEvent.end ? parseDateTime(editingEvent.date, editingEvent.end) : null;
      await call('event_add', {
        title: editingEvent.title,
        date: editingEvent.date,
        start: editingEvent.start,
        end: editingEvent.end,
        remindMin: editingEvent.remindMin,
        color: editingEvent.color
      });
      showEventForm = false;
      editingEvent = null;
      await refresh();
      toast('Event added');
    } catch (e) {
      console.error('Failed to add event:', e);
      toast(e.message || 'Failed to add event');
    }
  }

  async function updateEvent() {
    try {
      await call('event_update', {
        id: editingEvent.id,
        title: editingEvent.title,
        date: editingEvent.date,
        start: editingEvent.start,
        end: editingEvent.end,
        remindMin: editingEvent.remindMin,
        color: editingEvent.color
      });
      editingEvent = null;
      showEventForm = false;
      await refresh();
      toast('Event updated');
    } catch (e) {
      console.error('Failed to update event:', e);
      toast(e.message || 'Failed to update event');
    }
  }

  async function deleteEvent(event) {
    const original = { ...event };
    try {
      await call('event_delete', { id: event.id });
      undoStack.push({ type: 'event', original });
      setTimeout(() => { undoStack = undoStack.filter(u => u !== undoStack[undoStack.length - 1]); }, 5000);
      await refresh();
      toast('Event deleted (Undo available for 5s)');
    } catch (e) {
      console.error('Failed to delete event:', e);
      toast(e.message || 'Failed to delete event');
    }
  }

  function undo() {
    const last = undoStack[undoStack.length - 1];
    if (!last) return;
    
    if (last.type === 'event') {
      restoreEvent(last.original);
    } else if (last.type === 'task') {
      restoreTask(last.original);
    }
    undoStack = undoStack.slice(0, -1);
  }

  async function restoreEvent(original) {
    try {
      await call('event_add', {
        title: original.title,
        date: new Date(original.start * 1000).toISOString().split('T')[0],
        start: new Date(original.start * 1000).toTimeString().split(' ')[0].slice(0, 5),
        end: original.end ? new Date(original.end * 1000).toTimeString().split(' ')[0].slice(0, 5) : null,
        remindMin: original.remind_min,
        color: original.color
      });
      await refresh();
      toast('Event restored');
    } catch (e) {
      console.error('Failed to restore event:', e);
      toast('Failed to restore event');
    }
  }

  async function restoreTask(original) {
    try {
      await call('task_add_full', {
        title: original.title,
        day: original.day,
        due: original.due,
        remindMin: original.remind_min
      });
      await refresh();
      toast('Task restored');
    } catch (e) {
      console.error('Failed to restore task:', e);
      toast('Failed to restore task');
    }
  }

  // Task actions
  async function addTask(title, dueTimeStr, remindMin) {
    try {
      const now = new Date();
      const day = taskHorizon === 'today' ? todayStr : new Date(now.getTime() + 86400000).toISOString().split('T')[0];
      
      let due = null;
      if (dueTimeStr) {
        const [hours, minutes] = dueTimeStr.split(':').map(Number);
        const dueDate = taskHorizon === 'today' ? now : new Date(now.getTime() + 86400000);
        dueDate.setHours(hours, minutes, 0, 0);
        due = Math.floor(dueDate.getTime() / 1000);
      }
      
      await call('task_add_full', {
        title,
        day,
        due,
        remindMin: remindMin || 15
      });
      await refresh();
      toast('Task added');
    } catch (e) {
      console.error('Failed to add task:', e);
      toast(e.message || 'Failed to add task');
    }
  }

  async function toggleTask(task) {
    try {
      const currentNow = Math.floor(Date.now() / 1000);
      await call('task_toggle', { id: task.id, nowTs: currentNow });
      await refresh();
    } catch (e) {
      console.error('Failed to toggle task:', e);
      toast(e.message || 'Failed to toggle task');
    }
  }

  async function deleteTask(task) {
    const original = { ...task };
    try {
      await call('task_delete', { id: task.id });
      undoStack.push({ type: 'task', original });
      setTimeout(() => { undoStack = undoStack.filter(u => u !== undoStack[undoStack.length - 1]); }, 5000);
      await refresh();
      toast('Task deleted (Undo available for 5s)');
    } catch (e) {
      console.error('Failed to delete task:', e);
      toast(e.message || 'Failed to delete task');
    }
  }

  function handleKeyDown(e) {
    if (e.key === 'Escape') {
      showEventForm = false;
      editingEvent = null;
      editingTask = null;
    } else if (e.key === ' ' && editingTask && !editingTask.done) {
      e.preventDefault();
      toggleTask(editingTask);
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="task-calendar">
  <!-- Left: Month Grid -->
  <div class="calendar-section">
    <div class="calendar-header">
      <button class="icon-btn" onclick={prevMonth} aria-label="Previous month">
        <Icon name="chevron-left" size={16} />
      </button>
      <span class="month-name">
        {selectedDateObj.toLocaleDateString('en-US', { month: 'long', year: 'numeric' })}
      </span>
      <button class="icon-btn" onclick={nextMonth} aria-label="Next month">
        <Icon name="chevron-right" size={16} />
      </button>
    </div>
    {#if !isToday}
      <button class="today-btn" onclick={goToday}>Today</button>
    {/if}
    <div class="calendar-grid">
      <div class="day-header">Sun</div>
      <div class="day-header">Mon</div>
      <div class="day-header">Tue</div>
      <div class="day-header">Wed</div>
      <div class="day-header">Thu</div>
      <div class="day-header">Fri</div>
      <div class="day-header">Sat</div>
      {#each calendarDays as day (day?.date)}
        {@const isSelected = day?.date === selectedDate}
        <button
          class="calendar-day"
          class:selected={isSelected}
          class:today={day?.isToday}
          class:has-event={day?.hasEvent}
          class:padding={day === null}
          onclick={() => day && selectDay(day.date)}
          disabled={day === null}
          aria-label={day ? `Select ${day.isToday ? 'today, ' : ''}${day.date}` : ''}
        >
          {day?.day}
          {#if day?.hasEvent}
            <div class="event-dot"></div>
          {/if}
        </button>
      {/each}
    </div>
  </div>

  <!-- Right: Events and Tasks -->
  <div class="content-section">
    <!-- Events Section -->
    <div class="events-section">
      <div class="section-header">
        <span class="section-title">Events - {isToday ? 'Today' : selectedDate}</span>
        <button class="icon-btn add-btn" onclick={openEventForm} aria-label="Add event">
          <Icon name="plus" size={16} />
        </button>
      </div>
      
      {#if showEventForm}
        <div class="inline-form">
          <input 
            type="text" 
            placeholder="Event title" 
            bind:value={$editingEvent.title} 
          />
          <div class="form-row">
            <input 
              type="date" 
              bind:value={$editingEvent.date} 
            />
            <input 
              type="time" 
              placeholder="Start" 
              bind:value={$editingEvent.start} 
            />
            <input 
              type="time" 
              placeholder="End" 
              bind:value={$editingEvent.end} 
            />
          </div>
          <div class="form-row">
            <input 
              type="number" 
              placeholder="Reminder (min)" 
              bind:value={$editingEvent.remindMin} 
              min="0"
            />
            <div class="color-swatches">
              {#each COLOR_SWATCHES as color}
                <button 
                  class="color-swatch" 
                  style={`background: ${color}`}
                  class:selected={$editingEvent.color === color}
                  onclick={() => $editingEvent.color = color}
                  aria-label={`Select color ${color}`}
                ></button>
              {/each}
            </div>
          </div>
          <div class="form-actions">
            <button class="btn-secondary" onclick={() => { showEventForm = false; editingEvent = null; }}>Cancel</button>
            <button class="btn-primary" onclick={() => $editingEvent?.id ? updateEvent() : addEvent()}>
              {$editingEvent?.id ? 'Update' : 'Add'}
            </button>
          </div>
        </div>
      {:else if events.length === 0}
        <div class="empty-state">No events</div>
      {:else}
        <div class="events-list">
          {#each events as event (event.id)}
            <div 
              class="event-row" 
              class:in-progress={event.in_progress}
              role="button"
              tabindex="0"
              onclick={() => {
                const startDate = new Date(event.start * 1000);
                const dateStr = startDate.toISOString().split('T')[0];
                const startTime = startDate.toTimeString().split(' ')[0].slice(0, 5);
                const endTime = event.end ? new Date(event.end * 1000).toTimeString().split(' ')[0].slice(0, 5) : '';
                editingEvent = { 
                  ...event, 
                  date: dateStr, 
                  start: startTime, 
                  end: endTime 
                };
                showEventForm = true;
              }}
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.target.click(); } }}
              aria-label={`Edit event: ${event.title}`}
            >
              <div class="event-time">
                {formatTime(event.start)}
                {#if event.end}
                  <span class="time-sep">-</span>
                  {formatTime(event.end)}
                {/if}
              </div>
              <div class="event-content">
                <div class="event-title">{event.title}</div>
                {#if event.in_progress}
                  <span class="now-tag">now</span>
                {/if}
              </div>
              <div class="event-color" style={`background: ${event.color || 'var(--accent)'}`}></div>
              <button class="delete-btn" onclick={(e) => { e.stopPropagation(); deleteEvent(event); }} aria-label="Delete">
                <Icon name="x" size={12} />
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Tasks Section -->
    <div class="tasks-section">
      <div class="section-header">
        <div class="task-pills">
          <button 
            class="task-pill" 
            class:active={taskHorizon === 'today'}
            onclick={() => taskHorizon = 'today'}
          >
            Today
          </button>
          <button 
            class="task-pill" 
            class:active={taskHorizon === 'tomorrow'}
            onclick={() => taskHorizon = 'tomorrow'}
          >
            Tomorrow
          </button>
        </div>
      </div>
      
      {#if taskHorizon === 'today' && todayTasks.length === 0}
        <div class="empty-state">Nothing to do</div>
      {:else if taskHorizon === 'tomorrow' && tomorrowTasks.length === 0}
        <div class="empty-state">Nothing to do</div>
      {:else}
        <div class="tasks-list">
          {#each (taskHorizon === 'today' ? todayTasks : tomorrowTasks) as task (task.id)}
            <div 
              class="task-row" 
              class:done={task.done}
              class:overdue={task.overdue}
              role="button"
              tabindex="0"
              onclick={() => editingTask = { ...task }}
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.target.click(); } }}
              aria-label={`Edit task: ${task.title}`}
            >
              <input 
                type="checkbox" 
                checked={task.done}
                onclick={(e) => { e.stopPropagation(); toggleTask(task); }}
                aria-label="Toggle task"
              />
              <div class="task-content">
                <div class="task-title {task.done ? 'struck' : ''}">{task.title}</div>
                {#if task.due}
                  <div class="task-due">{formatTime(task.due)}</div>
                {/if}
                {#if task.overdue}
                  <span class="overdue-tag">overdue</span>
                {/if}
                {#if task.done}
                  <span class="done-tag">done</span>
                {/if}
              </div>
              <button class="delete-btn" onclick={(e) => { e.stopPropagation(); deleteTask(task); }} aria-label="Delete">
                <Icon name="x" size={12} />
              </button>
            </div>
          {/each}
        </div>
      {/if}
      
      <!-- Add Task Input -->
      <div class="add-task-row">
        <input 
          type="text" 
          placeholder="Add task (e.g., '17:30 Submit report' for due time)" 
          bind:value={$newTaskInput}
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              const text = $newTaskInput.trim();
              if (text) {
                const match = text.match(/(\d{1,2}):(\d{2})\s+(.+)/);
                if (match) {
                  const [, hours, minutes, title] = match;
                  addTask(title, `${hours}:${minutes}`, 15);
                } else {
                  addTask(text, null, 15);
                }
                $newTaskInput = '';
              }
            }
          }}
        />
        <div class="reminder-chip">
          <Icon name="bell" size={12} />
          <span>15m</span>
        </div>
      </div>
    </div>
  </div>
</div>

{#if undoStack.length > 0}
  <div class="undo-toast">
    <button onclick={undo}>Undo</button>
  </div>
{/if}

<style>
  .task-calendar {
    display: flex;
    height: 100%;
    gap: 16px;
    padding: 12px;
    font-size: 13px;
  }

  .calendar-section {
    width: 190px;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--row-hi);
    padding-right: 16px;
  }

  .calendar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .month-name {
    font-weight: 500;
    font-size: 14px;
  }

  .today-btn {
    font-size: 12px;
    color: var(--accent);
    background: none;
    border: none;
    cursor: pointer;
    padding: 4px 0;
    margin-bottom: 8px;
  }

  .today-btn:hover {
    text-decoration: underline;
  }

  .calendar-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 4px;
    flex: 1;
  }

  .day-header {
    font-size: 10px;
    color: var(--dim);
    text-align: center;
    font-weight: 500;
  }

  .calendar-day {
    aspect-ratio: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
    position: relative;
    border: none;
    background: transparent;
    padding: 0;
    color: inherit;
  }

  .calendar-day.padding {
    visibility: hidden;
    pointer-events: none;
  }

  .calendar-day:hover {
    background: var(--row-hi);
  }

  .calendar-day.selected {
    background: var(--accent);
    color: #0a0a0a;
  }

  .calendar-day.today {
    border: 1px solid var(--accent);
  }

  .calendar-day.has-event::after {
    content: '';
    position: absolute;
    bottom: 4px;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--accent);
  }

  .content-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }

  .events-section,
  .tasks-section {
    display: flex;
    flex-direction: column;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .section-title {
    font-weight: 500;
    font-size: 13px;
  }

  .task-pills {
    display: flex;
    gap: 6px;
  }

  .task-pill {
    padding: 4px 12px;
    background: var(--row);
    border: 1px solid var(--row-hi);
    border-radius: 999px;
    font-size: 12px;
    cursor: pointer;
    color: var(--dim);
  }

  .task-pill:hover {
    background: var(--row-hi);
  }

  .task-pill.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #0a0a0a;
  }

  .events-list,
  .tasks-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .event-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px;
    background: var(--row);
    border-radius: 8px;
    cursor: pointer;
    position: relative;
  }

  .event-row:hover {
    background: var(--row-hi);
  }

  .event-row.in-progress {
    border-left: 2px solid var(--accent);
  }

  .event-time {
    font-size: 11px;
    color: var(--dim);
    min-width: 70px;
  }

  .time-sep {
    margin: 0 4px;
    color: var(--dim);
  }

  .event-content {
    flex: 1;
    min-width: 0;
  }

  .event-title {
    font-size: 13px;
    font-weight: 500;
  }

  .now-tag {
    font-size: 10px;
    background: var(--accent);
    color: #0a0a0a;
    padding: 2px 6px;
    border-radius: 4px;
    margin-left: 6px;
  }

  .event-color {
    width: 4px;
    height: 16px;
    border-radius: 2px;
  }

  .task-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px;
    background: var(--row);
    border-radius: 8px;
    cursor: pointer;
  }

  .task-row:hover {
    background: var(--row-hi);
  }

  .task-row.done .task-title {
    text-decoration: line-through;
    color: var(--dim);
  }

  .task-row.done {
    opacity: 0.6;
  }

  .task-content {
    flex: 1;
    min-width: 0;
  }

  .task-title {
    font-size: 13px;
    font-weight: 500;
  }

  .task-due {
    font-size: 11px;
    color: var(--dim);
  }

  .overdue-tag {
    font-size: 10px;
    background: rgba(255, 138, 128, 0.15);
    color: #ff8a80;
    padding: 2px 6px;
    border-radius: 4px;
    margin-left: 6px;
  }

  .done-tag {
    font-size: 10px;
    color: var(--dim);
    margin-left: 6px;
  }

  .delete-btn {
    width: 20px;
    height: 20px;
    border: none;
    background: transparent;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: var(--dim);
    opacity: 0;
    transition: opacity 0.2s;
  }

  .event-row:hover .delete-btn,
  .task-row:hover .delete-btn {
    opacity: 1;
  }

  .delete-btn:hover {
    color: #ff8a80;
    background: rgba(255, 138, 128, 0.1);
  }

  .inline-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    background: var(--row);
    border-radius: 8px;
  }

  .inline-form input {
    padding: 6px 8px;
    border: 1px solid var(--row-hi);
    border-radius: 4px;
    background: var(--bg);
    color: var(--ink);
    font-size: 12px;
  }

  .form-row {
    display: flex;
    gap: 8px;
  }

  .form-row input {
    flex: 1;
  }

  .color-swatches {
    display: flex;
    gap: 6px;
  }

  .color-swatch {
    width: 20px;
    height: 20px;
    border-radius: 4px;
    cursor: pointer;
    border: 2px solid transparent;
    padding: 0;
  }

  .color-swatch.selected {
    border-color: var(--ink);
  }

  .form-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .btn-primary,
  .btn-secondary {
    padding: 6px 12px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
  }

  .btn-primary {
    background: var(--accent);
    color: #0a0a0a;
    border: none;
  }

  .btn-secondary {
    background: var(--row-hi);
    color: var(--ink);
    border: none;
  }

  .empty-state {
    text-align: center;
    color: var(--dim);
    padding: 24px 16px;
    font-size: 12px;
  }

  .add-task-row {
    display: flex;
    gap: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--row-hi);
  }

  .add-task-row input {
    flex: 1;
    padding: 6px 8px;
    border: 1px solid var(--row-hi);
    border-radius: 4px;
    background: var(--bg);
    color: var(--ink);
    font-size: 12px;
  }

  .reminder-chip {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px;
    background: var(--row-hi);
    border-radius: 999px;
    font-size: 11px;
    color: var(--dim);
  }

  .icon-btn {
    width: 24px;
    height: 24px;
    border: none;
    background: transparent;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: var(--dim);
  }

  .icon-btn:hover {
    background: var(--row-hi);
    color: var(--ink);
  }

  .add-btn {
    background: var(--accent);
    color: #0a0a0a;
  }

  .add-btn:hover {
    opacity: 0.9;
  }

  .undo-toast {
    position: fixed;
    bottom: 20px;
    right: 20px;
    background: var(--row);
    border: 1px solid var(--row-hi);
    border-radius: 8px;
    padding: 8px 12px;
    z-index: 1000;
  }

  .undo-toast button {
    background: var(--accent);
    color: #0a0a0a;
    border: none;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
  }
</style>
