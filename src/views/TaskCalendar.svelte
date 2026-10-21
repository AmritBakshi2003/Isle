<!--
  TaskCalendar.svelte: NotchNook-style simplified Tasks & Calendar
  Week strip calendar, events for selected day, plain task list
-->
<script>
  import { call, on } from '../lib/api.js';
  import { toast } from '../lib/state.svelte.js';
  import Icon from '../lib/Icon.svelte';
  import { onMount, untrack } from 'svelte';

  const EVENT_COLOR = 'var(--agenda-event)';
  const TASK_COLOR = 'var(--agenda-task)';
  const WEEK_START = 1; // Monday

  // State
  let events = $state([]);
  let eventDays = $state([]);
  let tasks = $state([]);
  let selectedDate = $state(new Date().toISOString().split('T')[0]);
  let calendarExpanded = $state(false);
  let showEventPopup = $state(false);
  let editingEvent = $state(null);
  let weekOffset = $state(0); // Week offset from today
  // +1 when moving forward, -1 when moving back: the strip slides the way you go
  let weekDir = $state(1);

  // Event form state
  let eventTitle = $state('');
  let eventDate = $state('');
  let eventHour = $state(1);
  let eventMinute = $state('00');
  let eventAmPm = $state('PM');
  let eventRemindMin = $state(10); // Default 10 min

  // Task input state
  let newTaskInput = $state('');

  // Timezone helper
  function tzOffsetMin() {
    return -new Date().getTimezoneOffset();
  }

  // Today's local date
  const todayStr = (() => {
    const now = new Date();
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
  })();

  // Derived: selected date as Date object
  let selectedDateObj = $derived.by(() => {
    const [year, month, day] = selectedDate.split('-').map(Number);
    return new Date(year, month - 1, day);
  });

  // Derived: is selected date today?
  let isToday = $derived(selectedDate === todayStr);

  // Derived: week strip days (7 days centered on today's date with offset)
  let weekDays = $derived.by(() => {
    const now = new Date();
    const date = new Date(now.getFullYear(), now.getMonth(), now.getDate() + (weekOffset * 7));
    const dayOfWeek = date.getDay();
    const diff = (dayOfWeek - WEEK_START + 7) % 7;
    date.setDate(date.getDate() - diff);
    
    const days = [];
    for (let i = 0; i < 7; i++) {
      const dateStr = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
      days.push({
        day: date.getDate(),
        date: dateStr,
        isToday: dateStr === todayStr,
        hasEvent: eventDays.includes(dateStr)
      });
      date.setDate(date.getDate() + 1);
    }
    return days;
  });

  // Derived: full month calendar days
  let calendarDays = $derived.by(() => {
    const year = selectedDateObj.getFullYear();
    const month = selectedDateObj.getMonth();
    
    const firstDay = new Date(year, month, 1);
    const lastDay = new Date(year, month + 1, 0);
    const daysInMonth = lastDay.getDate();
    
    let startDay = firstDay.getDay() - WEEK_START;
    if (startDay < 0) startDay += 7;
    
    const days = [];
    for (let i = 0; i < startDay; i++) days.push(null);
    for (let i = 1; i <= daysInMonth; i++) {
      const dateStr = `${year}-${String(month + 1).padStart(2, '0')}-${String(i).padStart(2, '0')}`;
      days.push({
        day: i,
        date: dateStr,
        isToday: dateStr === todayStr,
        hasEvent: eventDays.includes(dateStr)
      });
    }
    return days;
  });

  // Derived: events for selected day
  let dayEvents = $derived.by(() => {
    return events.filter(e => {
      const eventDate = new Date(e.start * 1000);
      const eventDateStr = `${eventDate.getFullYear()}-${String(eventDate.getMonth() + 1).padStart(2, '0')}-${String(eventDate.getDate()).padStart(2, '0')}`;
      return eventDateStr === selectedDate;
    });
  });

  // Derived: weekday letters
  const weekdayLetters = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];

  // 12-hour to 24-hour conversion
  function to12hTo24h(hour12, minute, isPM) {
    let hour24 = hour12;
    if (isPM && hour12 !== 12) hour24 += 12;
    if (!isPM && hour12 === 12) hour24 = 0;
    return hour24 * 60 + parseInt(minute);
  }

  // Format time in 12-hour format
  function formatTime24To12h(timestamp) {
    const date = new Date(timestamp * 1000);
    let hours = date.getHours();
    const minutes = date.getMinutes();
    const ampm = hours >= 12 ? 'PM' : 'AM';
    hours = hours % 12;
    hours = hours || 12;
    return `${hours}:${String(minutes).padStart(2, '0')} ${ampm}`;
  }

  // Load functions
  async function loadEvents() {
    try {
      events = await call('events_for_day', {
        date: selectedDate,
        tzOffsetMin: tzOffsetMin()
      });
    } catch (e) {
      console.error('Failed to load events:', e);
      toast.error(e);
    }
  }

  async function loadEventDays() {
    try {
      const year = selectedDateObj.getFullYear();
      const month = selectedDateObj.getMonth() + 1;
      eventDays = await call('month_event_days', {
        year,
        month,
        tzOffsetMin: tzOffsetMin()
      });
    } catch (e) {
      console.error('Failed to load event days:', e);
      toast.error(e);
    }
  }

  async function loadTasks() {
    try {
      tasks = await call('task_list');
    } catch (e) {
      console.error('Failed to load tasks:', e);
      toast.error(e);
    }
  }

  async function refresh() {
    await Promise.all([loadEvents(), loadEventDays(), loadTasks()]);
  }

  // Event handlers
  function selectDay(date) {
    selectedDate = date;
    refresh();
  }

  function prevWeek() {
    weekDir = -1;
    weekOffset -= 1;
  }

  function nextWeek() {
    weekDir = 1;
    weekOffset += 1;
  }

  function prevMonth() {
    const date = new Date(selectedDateObj);
    date.setMonth(date.getMonth() - 1);
    selectedDate = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
    refresh();
  }

  function nextMonth() {
    const date = new Date(selectedDateObj);
    date.setMonth(date.getMonth() + 1);
    selectedDate = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
    refresh();
  }

  function goToday() {
    selectedDate = todayStr;
    calendarExpanded = false;
    refresh();
  }

  function openEventPopup(event = null) {
    editingEvent = event;
    showEventPopup = true;
    if (event) {
      eventTitle = event.title;
      eventDate = selectedDate;
      const date = new Date(event.start * 1000);
      let hours = date.getHours();
      const minutes = date.getMinutes();
      eventAmPm = hours >= 12 ? 'PM' : 'AM';
      hours = hours % 12;
      hours = hours || 12;
      eventHour = hours;
      eventMinute = String(minutes).padStart(2, '0');
      eventRemindMin = event.remind_min;
    } else {
      eventTitle = '';
      eventDate = selectedDate;
      // Default to next full hour
      const now = new Date();
      let nextHour = now.getHours() + 1;
      eventAmPm = nextHour >= 12 ? 'PM' : 'AM';
      nextHour = nextHour % 12;
      nextHour = nextHour || 12;
      eventHour = nextHour;
      eventMinute = '00';
      eventRemindMin = 10;
    }
  }

  function closeEventPopup() {
    showEventPopup = false;
    editingEvent = null;
  }

  async function saveEvent() {
    if (!eventTitle.trim()) {
      toast.error('Event title is required');
      return;
    }
    
    try {
      const minutes = to12hTo24h(eventHour, eventMinute, eventAmPm === 'PM');
      const [year, month, day] = eventDate.split('-').map(Number);
      const eventDateObj = new Date(year, month - 1, day);
      eventDateObj.setHours(Math.floor(minutes / 60));
      eventDateObj.setMinutes(minutes % 60);
      eventDateObj.setSeconds(0);
      const startTimestamp = Math.floor(eventDateObj.getTime() / 1000);
      
      if (editingEvent) {
        await call('event_update', {
          id: editingEvent.id,
          title: eventTitle,
          start: startTimestamp,
          end: null,
          remindMin: eventRemindMin,
          color: null
        });
      } else {
        await call('event_add', {
          title: eventTitle,
          start: startTimestamp,
          end: null,
          remindMin: eventRemindMin,
          color: null
        });
      }
      
      closeEventPopup();
      refresh();
    } catch (e) {
      console.error('Failed to save event:', e);
      toast.error(e);
    }
  }

  async function deleteEvent(id) {
    try {
      await call('event_delete', { id });
      refresh();
    } catch (e) {
      console.error('Failed to delete event:', e);
      toast.error(e);
    }
  }

  async function addTask() {
    const title = newTaskInput.value.trim();
    if (!title) return;
    
    try {
      await call('task_add', { title });
      newTaskInput.value = '';
      newTaskInput.focus();
      loadTasks();
    } catch (e) {
      console.error('Failed to add task:', e);
      toast.error(e);
    }
  }

  async function toggleTask(id) {
    try {
      await call('task_toggle', { id });
      loadTasks();
    } catch (e) {
      console.error('Failed to toggle task:', e);
      toast.error(e);
    }
  }

  async function deleteTask(id) {
    try {
      await call('task_delete', { id });
      loadTasks();
    } catch (e) {
      console.error('Failed to delete task:', e);
      toast.error(e);
    }
  }

  async function clearCompleted() {
    try {
      const count = await call('task_clear_done');
      loadTasks();
      toast.success(`Cleared ${count} completed tasks`);
    } catch (e) {
      console.error('Failed to clear completed:', e);
      toast.error(e);
    }
  }

  // Reminder chip options
  const reminderOptions = [
    { label: 'Off', value: 0 },
    { label: '5 min', value: 5 },
    { label: '10 min', value: 10 },
    { label: '15 min', value: 15 },
    { label: '30 min', value: 30 },
    { label: '1 hr', value: 60 }
  ];

  // Derived: sorted tasks (done at bottom)
  let sortedTasks = $derived.by(() => {
    return [...tasks].sort((a, b) => a.done - b.done || b.id - a.id);
  });

  // Derived: has any done tasks
  let hasDoneTasks = $derived(tasks.some(t => t.done));

  // On mount
  onMount(() => {
    refresh();
    
    // Visibility/focus handler
    const handleVisibilityChange = () => refresh();
    document.addEventListener('visibilitychange', handleVisibilityChange);
    
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  });

  // Test helper for to12hTo24h
  function testTo12hTo24h() {
    console.assert(to12hTo24h(12, '00', false) === 0, '12:00 AM should be 0');
    console.assert(to12hTo24h(12, '30', false) === 30, '12:30 AM should be 30');
    console.assert(to12hTo24h(12, '00', true) === 720, '12:00 PM should be 720');
    console.assert(to12hTo24h(1, '00', true) === 780, '1:00 PM should be 780');
    console.assert(to12hTo24h(11, '59', true) === 1439, '11:59 PM should be 1439');
  }
  testTo12hTo24h();
</script>

<svelte:boundary>
  <div class="task-calendar" class:calendar-expanded={calendarExpanded}>
    <!-- Calendar Header (always visible) -->
    <div class="calendar-header">
      <span class="month-name">
        {selectedDateObj.toLocaleDateString('en-US', { month: 'short' })}
      </span>
      <div class="header-right">
        <button class="event-btn" onclick={() => openEventPopup()}>
          <Icon name="plus" size={12} />
          Event
        </button>
        <button 
          class="expand-btn" 
          onclick={() => calendarExpanded = !calendarExpanded}
          aria-expanded={calendarExpanded}
          aria-label={calendarExpanded ? 'Collapse calendar' : 'Expand calendar'}
        >
          <Icon name={calendarExpanded ? 'chevron-up' : 'chevron-down'} size={14} />
        </button>
        {#if !isToday}
          <button class="today-btn" onclick={goToday}>Today</button>
        {/if}
      </div>
    </div>

    <!-- Week Strip (collapsed) or Full Month (expanded) -->
    {#if !calendarExpanded}
      <!-- Nav buttons stay put; only the week cells slide, so the arrows never move
           under the cursor. overflow:hidden keeps the strip inside the panel. -->
      <div class="week-row">
        <button class="nav-btn" onclick={prevWeek} aria-label="Previous week">
          <Icon name="chevron-left" size={14} />
        </button>
        <div class="week-viewport">
          {#key weekOffset}
            <div
              class="week-strip"
              style:--from={weekDir * 26}px
            >
              {#each weekDays as day, i (day.date)}
                <button 
                  class="day-cell"
                  class:today={day.isToday}
                  class:selected={day.date === selectedDate}
                  onclick={() => selectDay(day.date)}
                  aria-label={day.isToday ? 'Today' : day.date}
                >
                  <span class="weekday">{weekdayLetters[i]}</span>
                  <span class="day-number">{day.day}</span>
                  {#if day.hasEvent}
                    <div class="event-dot"></div>
                  {/if}
                </button>
              {/each}
            </div>
          {/key}
        </div>
        <button class="nav-btn" onclick={nextWeek} aria-label="Next week">
          <Icon name="chevron-right" size={14} />
        </button>
      </div>
    {:else}
      <div class="expanded-layout">
        <!-- Calendar on left -->
        <div class="calendar-left">
          <div class="month-header">
            <button class="nav-btn" onclick={prevMonth} aria-label="Previous month">
              <Icon name="chevron-left" size={14} />
            </button>
            <span class="month-name-full">
              {selectedDateObj.toLocaleDateString('en-US', { month: 'long', year: 'numeric' })}
            </span>
            <button class="nav-btn" onclick={nextMonth} aria-label="Next month">
              <Icon name="chevron-right" size={14} />
            </button>
          </div>
          <div class="weekday-row">
            {#each weekdayLetters as letter}
              <span class="weekday-header">{letter}</span>
            {/each}
          </div>
          <div class="calendar-days">
            {#each calendarDays as day, i (day?.date || i)}
              {#if day}
                <button 
                  class="calendar-day"
                  class:today={day.isToday}
                  class:selected={day.date === selectedDate}
                  onclick={() => selectDay(day.date)}
                  aria-label={day.isToday ? 'Today' : day.date}
                >
                  {day.day}
                  {#if day.hasEvent}
                    <div class="event-dot"></div>
                  {/if}
                </button>
              {:else}
                <div class="calendar-day padding"></div>
              {/if}
            {/each}
          </div>
        </div>

        <!-- Events and Tasks on right -->
        <div class="events-section-right">
          <!-- Events -->
          {#if dayEvents.length === 0}
            <div class="empty-state">
              <Icon name="calendar" size={16} />
              <span>{isToday ? 'Nothing for today' : `Nothing on ${selectedDate}`}</span>
            </div>
          {:else}
            {#each dayEvents as event (event.id)}
              <div 
                class="event-row"
                role="button"
                tabindex="0"
                onclick={() => openEventPopup(event)}
                onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openEventPopup(event); }}}
                title={event.title}
              >
                <div class="event-color-bar" style="background: {EVENT_COLOR}"></div>
                <span class="event-time">{formatTime24To12h(event.start)}</span>
                <span class="event-title">{event.title}</span>
                <button 
                  class="delete-btn"
                  onclick={(e) => { e.stopPropagation(); deleteEvent(event.id); }}
                  aria-label="Delete"
                  type="button"
                >
                  <Icon name="close" size={12} />
                </button>
              </div>
            {/each}
          {/if}

          <!-- Tasks -->
          <div class="tasks-inline">
            <div class="task-input-row">
              <Icon name="plus" size={16} style="color: {TASK_COLOR}" />
              <input 
                type="text" 
                class="task-input" 
                placeholder="New task..." 
                bind:this={newTaskInput}
                onkeydown={(e) => { if (e.key === 'Enter') addTask(); }}
              />
              {#if hasDoneTasks}
                <button class="clear-btn" onclick={clearCompleted}>Clear completed</button>
              {/if}
            </div>
            
            <div class="tasks-list">
              {#if sortedTasks.length === 0}
                <div class="empty-state">No tasks</div>
              {:else}
                {#each sortedTasks as task (task.id)}
                  <div 
                    class="task-row"
                    class:done={task.done}
                    title={task.title}
                  >
                    <input 
                      type="checkbox" 
                      class="task-checkbox"
                      checked={task.done}
                      onclick={() => toggleTask(task.id)}
                    />
                    <span class="task-title">{task.title}</span>
                    <button 
                      class="delete-btn"
                      onclick={() => deleteTask(task.id)}
                      aria-label="Delete"
                    >
                      <Icon name="close" size={12} />
                    </button>
                  </div>
                {/each}
              {/if}
            </div>
          </div>
        </div>
      </div>
    {/if}

    <!-- Events Section (collapsed view only) -->
    {#if !calendarExpanded}
      <div class="events-section">
        {#if dayEvents.length === 0}
          <div class="empty-state">
            <Icon name="calendar" size={16} />
            <span>{isToday ? 'Nothing for today' : `Nothing on ${selectedDate}`}</span>
          </div>
        {:else}
          {#each dayEvents as event (event.id)}
            <div 
              class="event-row"
              role="button"
              tabindex="0"
              onclick={() => openEventPopup(event)}
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openEventPopup(event); }}}
              title={event.title}
            >
              <div class="event-color-bar" style="background: {EVENT_COLOR}"></div>
              <span class="event-time">{formatTime24To12h(event.start)}</span>
              <span class="event-title">{event.title}</span>
              <button 
                class="delete-btn"
                onclick={(e) => { e.stopPropagation(); deleteEvent(event.id); }}
                aria-label="Delete"
                type="button"
              >
                <Icon name="close" size={12} />
              </button>
            </div>
          {/each}
        {/if}
      </div>
    {/if}

    <!-- Tasks Area (collapsed view only) -->
    {#if !calendarExpanded}
      <div class="tasks-area">
        <div class="task-input-row">
          <Icon name="plus" size={16} style="color: {TASK_COLOR}" />
          <input 
            type="text" 
            class="task-input" 
            placeholder="New task..." 
            bind:this={newTaskInput}
            onkeydown={(e) => { if (e.key === 'Enter') addTask(); }}
          />
          {#if hasDoneTasks}
            <button class="clear-btn" onclick={clearCompleted}>Clear completed</button>
          {/if}
        </div>
        
        <div class="tasks-list">
          {#if sortedTasks.length === 0}
            <div class="empty-state">No tasks</div>
          {:else}
            {#each sortedTasks as task (task.id)}
              <div 
                class="task-row"
                class:done={task.done}
                title={task.title}
              >
                <input 
                  type="checkbox" 
                  class="task-checkbox"
                  checked={task.done}
                  onclick={() => toggleTask(task.id)}
                />
                <span class="task-title">{task.title}</span>
                <button 
                  class="delete-btn"
                  onclick={() => deleteTask(task.id)}
                  aria-label="Delete"
                >
                  <Icon name="close" size={12} />
                </button>
              </div>
            {/each}
          {/if}
        </div>
      </div>
    {/if}
  </div>

  <!-- Event Popup Modal -->
  {#if showEventPopup}
    <div 
      class="modal-overlay" 
      role="button" 
      tabindex="-1" 
      onclick={(e) => { if (e.target === e.currentTarget) closeEventPopup(); }}
      onkeydown={(e) => { if (e.key === 'Escape') closeEventPopup(); }}
    >
      <div class="modal" role="dialog" tabindex="-1" inert={!showEventPopup}>
        <div class="modal-header">
          <span class="modal-title">{editingEvent ? 'Edit event' : 'New event'}</span>
        </div>
        <div class="modal-body">
          <input 
            type="text" 
            class="modal-input" 
            placeholder="Event title" 
            bind:value={eventTitle} 
            onkeydown={(e) => { if (e.key === 'Enter') saveEvent(); }}
          />
          <input 
            type="date" 
            class="modal-input" 
            bind:value={eventDate} 
          />
          <div class="time-row">
            <input 
              type="number" 
              class="time-input" 
              min="1" 
              max="12" 
              bind:value={eventHour} 
            />
            <span>:</span>
            <input 
              type="text" 
              class="time-input" 
              maxlength="2" 
              bind:value={eventMinute} 
            />
            <div class="ampm-toggle">
              <button 
                class:active={eventAmPm === 'AM'} 
                onclick={() => eventAmPm = 'AM'}
              >AM</button>
              <button 
                class:active={eventAmPm === 'PM'} 
                onclick={() => eventAmPm = 'PM'}
              >PM</button>
            </div>
          </div>
          <div class="reminder-section">
            <span class="reminder-label">Remind me before</span>
            <div class="reminder-chips">
              {#each reminderOptions as opt}
                <button 
                  class:active={eventRemindMin === opt.value}
                  onclick={() => eventRemindMin = opt.value}
                >
                  {opt.label}
                </button>
              {/each}
            </div>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn-secondary" onclick={closeEventPopup}>Cancel</button>
          <button class="btn-primary" onclick={saveEvent}>{editingEvent ? 'Save' : 'Add'}</button>
        </div>
      </div>
    </div>
  {/if}
</svelte:boundary>

<style>
  .task-calendar {
    --agenda-event: var(--accent);
    --agenda-task: hsl(175 50% 55%);
    
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    padding: 0 12px;
    gap: 6px;
    position: relative;
  }

  /* Calendar Header */
  .calendar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 30px;
    flex: 0 0 auto;
    z-index: 10;
  }

  .month-name {
    font-size: 20px;
    font-weight: 700;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .event-btn {
    height: 24px;
    padding: 0 10px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid var(--agenda-event);
    border-radius: 6px;
    font-size: 11px;
    color: var(--agenda-event);
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 4px;
    transition: all 0.15s;
  }

  .event-btn:hover {
    background: rgba(157, 180, 255, 0.12);
  }

  .expand-btn {
    width: 24px;
    height: 24px;
    border: none;
    background: transparent;
    border-radius: 4px;
    cursor: pointer;
    color: var(--dim);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .expand-btn:hover {
    background: var(--row-hi);
    color: var(--ink);
  }

  .today-btn {
    font-size: 11px;
    color: var(--accent);
    background: none;
    border: none;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
  }

  .today-btn:hover {
    background: var(--row-hi);
  }

  /* Week Strip */
  .week-row {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 46px;
    flex: 0 0 auto;
  }
  .week-viewport {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
  }
  .week-strip {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    will-change: transform, opacity;
    /* the new week slides in from the side you pressed; enter-only, so the
       previous strip disappears at once and can never linger as a ghost */
    animation: week-in .3s cubic-bezier(.22,.9,.3,1) both;
  }
  @keyframes week-in {
    from { opacity: 0; transform: translateX(var(--from, 26px)); }
    to   { opacity: 1; transform: none; }
  }

  .nav-btn {
    width: 28px;
    height: 28px;
    border: none;
    background: transparent;
    border-radius: 4px;
    cursor: pointer;
    color: var(--dim);
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
  }

  .nav-btn:hover {
    background: var(--row-hi);
    color: var(--ink);
  }

  .day-cell {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    border-radius: 8px;
    cursor: pointer;
    position: relative;
    min-width: 0;
  }

  .day-cell:hover {
    background: var(--row-hi);
  }

  .weekday {
    font-size: 9px;
    color: var(--dim);
    margin-bottom: 2px;
  }

  .day-number {
    font-size: 13px;
    font-weight: 500;
  }

  .day-cell.today .day-number {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--accent);
    color: #0a0a0a;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .day-cell.selected .day-number {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 1px solid var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .event-dot {
    position: absolute;
    bottom: 2px;
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--agenda-event);
  }

  .day-cell.today .event-dot {
    background: #0a0a0a;
  }

  /* Expanded Layout */
  .expanded-layout {
    display: flex;
    gap: 12px;
    flex: 0 0 auto;
    min-height: 0;
  }

  .calendar-left {
    flex: 0 0 45%;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    height: 100%;
  }

  .calendar-left .month-header {
    display: flex;
    align-items: center;
    height: 32px;
    gap: 4px;
  }

  .calendar-left .month-name-full {
    font-weight: 500;
    font-size: 14px;
    flex: 1;
    text-align: center;
  }

  .calendar-left .weekday-row {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 6px;
  }

  .calendar-left .weekday-header {
    font-size: 10px;
    color: var(--dim);
    text-align: center;
    font-weight: 500;
  }

  .calendar-left .calendar-days {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 6px;
  }

  .calendar-left .calendar-day {
    width: 30px;
    height: 30px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    cursor: pointer;
    font-size: 12px;
    position: relative;
    border: none;
    background: transparent;
    padding: 0;
    color: inherit;
    margin: 0 auto;
  }

  .calendar-left .calendar-day.padding {
    visibility: hidden;
  }

  .calendar-left .calendar-day:hover {
    background: var(--row-hi);
  }

  .calendar-left .calendar-day.today {
    background: var(--accent);
    color: #0a0a0a;
  }

  .calendar-left .calendar-day.selected {
    border: 1px solid var(--accent);
  }

  .calendar-left .event-dot {
    position: absolute;
    bottom: 1px;
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--agenda-event);
  }

  .calendar-left .calendar-day.today .event-dot {
    background: #0a0a0a;
  }

  .calendar-left .nav-btn {
    width: 20px;
    height: 20px;
    border: none;
    background: transparent;
    border-radius: 4px;
    cursor: pointer;
    color: var(--dim);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .calendar-left .nav-btn:hover {
    background: var(--row-hi);
    color: var(--ink);
  }

  .events-section-right {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow-y: auto;
  }

  /* hidden bars, still scrollable: a clean look was requested */
  .events-section-right {
    scrollbar-width: none;
  }

  .events-section-right::-webkit-scrollbar {
    width: 0;
    height: 0;
  }

  .tasks-inline {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tasks-inline .task-input-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 0 0 auto;
  }

  .tasks-inline .task-input {
    flex: 1;
    min-width: 0;
    height: 28px;
    background: var(--row);
    border: none;
    border-radius: 6px;
    padding: 0 10px;
    font-size: 13px;
    color: var(--ink);
  }

  .tasks-inline .task-input::placeholder {
    color: var(--dim);
  }

  .tasks-inline .task-input:focus {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .tasks-inline .clear-btn {
    font-size: 11px;
    color: var(--dim);
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 0;
    white-space: nowrap;
  }

  .tasks-inline .clear-btn:hover {
    color: var(--ink);
  }

  .tasks-inline .tasks-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tasks-inline .tasks-list {
    scrollbar-width: none;
  }

  .tasks-inline .tasks-list::-webkit-scrollbar {
    width: 0;
    height: 0;
  }

  .tasks-inline .task-row {
    height: 28px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 6px;
    border-radius: 4px;
    background: var(--row);
  }

  .tasks-inline .task-row:hover {
    background: var(--row-hi);
  }

  .tasks-inline .task-row.done .task-title {
    text-decoration: line-through;
    opacity: 0.5;
  }

  .tasks-inline .task-checkbox {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    cursor: pointer;
    accent-color: var(--agenda-task);
  }

  .tasks-inline .task-title {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tasks-inline .delete-btn {
    opacity: 0;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    color: var(--dim);
    border: none;
    background: transparent;
    cursor: pointer;
  }

  .tasks-inline .task-row:hover .delete-btn {
    opacity: 1;
  }

  .tasks-inline .delete-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--ink);
  }

  /* Events Section (normal view) */
  .events-section {
    flex: 0 0 auto;
    max-height: 80px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 0;
  }

  .events-section::-webkit-scrollbar {
    width: 6px;
  }

  .events-section::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.14);
    border-radius: 3px;
  }

  /* Hide events-section and tasks-area in expanded view */
  .task-calendar.calendar-expanded > .events-section,
  .task-calendar.calendar-expanded > .tasks-area {
    display: none;
  }

  .event-row {
    height: 24px;
    display: grid;
    grid-template-columns: 3px 50px minmax(0, 1fr) 20px;
    align-items: center;
    gap: 6px;
    padding: 0 6px;
    border-radius: 4px;
    background: var(--row);
    cursor: pointer;
  }

  .event-row:hover {
    background: var(--row-hi);
  }

  .event-row:focus {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .event-color-bar {
    width: 3px;
    height: 100%;
    border-radius: 1px;
  }

  .event-time {
    font-size: 11px;
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }

  .event-title {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .delete-btn {
    opacity: 0;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    color: var(--dim);
    border: none;
    background: transparent;
    cursor: pointer;
  }

  .event-row:hover .delete-btn {
    opacity: 1;
  }

  .delete-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--ink);
  }

  /* Tasks Area (collapsed view only) */
  .tasks-area {
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .task-input-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 0 0 auto;
  }

  .task-input {
    flex: 1;
    min-width: 0;
    height: 28px;
    background: var(--row);
    border: none;
    border-radius: 6px;
    padding: 0 10px;
    font-size: 13px;
    color: var(--ink);
  }

  .task-input:focus {
    outline: none;
    background: var(--row-hi);
  }

  .task-input::placeholder {
    color: var(--dim);
  }

  .clear-btn {
    font-size: 11px;
    color: var(--dim);
    background: none;
    border: none;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
  }

  .clear-btn:hover {
    background: var(--row-hi);
    color: var(--ink);
  }

  .tasks-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tasks-list::-webkit-scrollbar {
    width: 6px;
  }

  .tasks-list::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.14);
    border-radius: 3px;
  }

  .task-row {
    height: 28px;
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) 20px;
    align-items: center;
    gap: 6px;
    padding: 0 6px;
    border-radius: 4px;
    background: var(--row);
  }

  .task-row:hover {
    background: var(--row-hi);
  }

  .task-row.done .task-title {
    text-decoration: line-through;
    opacity: 0.6;
  }

  .task-checkbox {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    cursor: pointer;
  }

  .task-title {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .task-row:hover .delete-btn {
    opacity: 1;
  }

  /* Empty State */
  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    color: var(--dim);
    font-size: 12px;
    padding: 12px;
  }

  /* Modal */
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: #14161d;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    width: 300px;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .modal-header {
    padding: 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .modal-title {
    font-weight: 500;
    font-size: 14px;
  }

  .modal-body {
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .modal-input {
    background: var(--row);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 8px 10px;
    font-size: 13px;
    color: var(--ink);
  }

  .modal-input:focus {
    border-color: var(--accent);
    outline: none;
  }

  .time-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .time-input {
    width: 50px;
    background: var(--row);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 8px;
    font-size: 13px;
    color: var(--ink);
    text-align: center;
  }

  .ampm-toggle {
    display: flex;
    gap: 2px;
  }

  .ampm-toggle button {
    flex: 1;
    height: 32px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    background: var(--row);
    color: var(--ink);
    font-size: 11px;
    cursor: pointer;
  }

  .ampm-toggle button.active {
    background: var(--accent);
    color: #0a0a0a;
    border-color: var(--accent);
  }

  .reminder-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .reminder-label {
    font-size: 12px;
    color: var(--dim);
  }

  .reminder-chips {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }

  .reminder-chips button {
    height: 24px;
    padding: 0 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    background: var(--row);
    color: var(--ink);
    font-size: 11px;
    cursor: pointer;
  }

  .reminder-chips button.active {
    background: var(--accent);
    color: #0a0a0a;
    border-color: var(--accent);
  }

  .modal-footer {
    padding: 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .btn-secondary,
  .btn-primary {
    height: 28px;
    padding: 0 16px;
    border-radius: 6px;
    font-size: 12px;
    cursor: pointer;
  }

  .btn-secondary {
    background: var(--row);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: var(--ink);
  }

  .btn-secondary:hover {
    background: var(--row-hi);
  }

  .btn-primary {
    background: var(--accent);
    border: 1px solid var(--accent);
    color: #0a0a0a;
  }

  .btn-primary:hover {
    opacity: 0.9;
  }
</style>
