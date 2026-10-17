<!--
  UtilityDrawer.svelte: Minimalist centralized utility drawer
  Windows native tools and custom app shortcuts for quick access.
-->
<script>
  import { call } from '../lib/api.js';
  import { toast } from '../lib/state.svelte.js';
  import Icon from '../lib/Icon.svelte';

  // App shortcuts state
  let shortcuts = $state([]);
  let availableApps = $state([]);
  let showAppPicker = $state(false);
  let searchQuery = $state('');
  let highlightedIndex = $state(0);
  let appIconUrls = $state({});

  // Check if app is already added
  function isAppAdded(app) {
    const appPath = app.path;
    const appTarget = app.target;
    return shortcuts.some(s => 
      s.path === appPath || 
      (appTarget && s.target === appTarget)
    );
  }

  async function loadShortcuts() {
    try {
      shortcuts = await call('app_shortcuts_list');
    } catch (e) {
      console.error('Failed to load shortcuts:', e);
    }
  }
  loadShortcuts();

  async function loadAvailableApps() {
    try {
      availableApps = await call('enumerate_apps');
    } catch (e) {
      console.error('Failed to enumerate apps:', e);
      toast('Failed to load apps');
    }
  }

  async function addShortcut(app) {
    try {
      await call('app_shortcut_add', {
        name: app.name,
        path: app.path,
        icon: app.icon,
        target: app.target,
        group: app.group,
        hidden: app.hidden
      });
      toast(`Added ${app.name}`);
      await loadShortcuts();
      // Force reload icon URLs after adding new shortcut
      await loadIconUrls();
      showAppPicker = false;
    } catch (e) {
      console.error('Failed to add shortcut:', e);
      toast('Failed to add shortcut');
    }
  }

  async function removeShortcut(shortcut) {
    try {
      await call('app_shortcut_remove', { id: shortcut.id });
      // Clear icon URL from state
      if (iconUrls[shortcut.id]) {
        delete iconUrls[shortcut.id];
      }
      toast('Shortcut removed');
      await loadShortcuts();
    } catch (e) {
      console.error('Failed to remove shortcut:', e);
      toast('Failed to remove shortcut');
    }
  }

  async function launchShortcut(shortcut) {
    try {
      await call('app_shortcut_launch', { path: shortcut.path });
    } catch (e) {
      console.error('Failed to launch app:', e);
      toast('Failed to launch app');
    }
  }

  // Preload icon URLs for shortcuts
  let iconUrls = $state({});

  async function loadIconUrls() {
    const newUrls = {};
    for (const shortcut of shortcuts) {
      if (shortcut.icon_filename) {
        try {
          const url = await call('app_shortcut_icon', { iconFilename: shortcut.icon_filename });
          if (url) {
            newUrls[shortcut.id] = url;
          }
        } catch (e) {
          console.error('Failed to load icon:', e);
        }
      }
    }
    iconUrls = newUrls;
  }

  $effect(() => {
    loadIconUrls();
  });

  // Get appropriate icon for an app based on name
  function getAppIconName(shortcut) {
    const name = shortcut.name.toLowerCase();
    if (name.includes('chrome') || name.includes('edge')) return 'file-video'; // Using blue-ish icon
    if (name.includes('firefox')) return 'file-video'; // Orange icon
    if (name.includes('excel') || name.includes('spreadsheet')) return 'file-xls'; // Excel-like
    if (name.includes('word') || name.includes('document')) return 'file-doc'; // Word-like
    if (name.includes('powerpoint') || name.includes('presentation')) return 'file-pdf'; // Presentation-like
    if (name.includes('outlook') || name.includes('mail')) return 'file'; // Mail-like
    if (name.includes('visual studio') || name.includes('vscode') || name.includes('code')) return 'file-code'; // Code-like
    if (name.includes('spotify')) return 'music'; // Music
    if (name.includes('discord')) return 'system'; // Discord-like
    if (name.includes('slack')) return 'file'; // Slack-like
    if (name.includes('steam')) return 'system'; // Steam-like
    if (name.includes('photoshop') || name.includes('adobe')) return 'file-image'; // Image-like
    if (name.includes('recycle') || name.includes('trash')) return 'trash'; // Trash
    if (name.includes('control') || name.includes('settings')) return 'settings'; // Settings
    if (name.includes('calculator')) return 'calc'; // Calculator
    if (name.includes('notepad') || name.includes('text')) return 'file-doc'; // Text
    if (name.includes('7-zip') || name.includes('zip') || name.includes('archive')) return 'file-zip'; // Archive
    return 'system'; // Default
  }

  // Regenerate all icons
  async function regenerateIcons() {
    try {
      await call('clear_all_icons');
      // Clear icon URLs to force regeneration
      iconUrls = {};
      // Reload shortcuts will trigger icon extraction for each one
      await loadShortcuts();
      await loadIconUrls();
      toast('Icons regenerated');
    } catch (e) {
      console.error('Failed to regenerate icons:', e);
      toast('Failed to regenerate icons');
    }
  }

  function openAppPicker() {
    loadAvailableApps();
    showAppPicker = true;
    searchQuery = '';
    highlightedIndex = 0;
    appIconUrls = {};
    // Autofocus search input after modal opens
    setTimeout(() => {
      const searchInput = document.querySelector('.search-input');
      if (searchInput) searchInput.focus();
    }, 100);
  }

  // Load app icon lazily
  async function loadAppIcon(app) {
    if (appIconUrls[app.path]) return; // Already cached

    try {
      const icon = app.icon || app.path;
      const iconUrl = await call('extract_app_icon', { path: icon });
      if (iconUrl) {
        appIconUrls[app.path] = iconUrl;
      }
    } catch (e) {
      console.error('Failed to load app icon:', e);
    }
  }

  // Keyboard navigation
  function handleKeydown(e) {
    if (!showAppPicker) return;

    if (e.key === 'Escape') {
      showAppPicker = false;
      return;
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      highlightedIndex = Math.min(highlightedIndex + 1, filteredApps.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      highlightedIndex = Math.max(highlightedIndex - 1, 0);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (filteredApps[highlightedIndex]) {
        addShortcut(filteredApps[highlightedIndex]);
      }
    }
  }

  // Lazy load icons for visible apps
  $effect(() => {
    if (!showAppPicker) return;

    // Load icons for visible apps (first 20, or around highlighted index)
    const visibleIndices = [];
    for (let i = Math.max(0, highlightedIndex - 5); i < Math.min(filteredApps.length, highlightedIndex + 15); i++) {
      visibleIndices.push(i);
    }

    for (const idx of visibleIndices) {
      const app = filteredApps[idx];
      if (app) {
        loadAppIcon(app);
      }
    }
  });

  // Windows native tools
  function openCalc() {
    try {
      call('open_native', { app: 'calc' });
      toast('Calculator opened');
    } catch (e) {
      toast('Failed to open Calculator');
    }
  }

  function openNotepad() {
    try {
      call('open_native', { app: 'notepad' });
      toast('Notepad opened');
    } catch (e) {
      toast('Failed to open Notepad');
    }
  }

  function openSettings() {
    try {
      call('open_native', { app: 'settings' });
      toast('Settings opened');
    } catch (e) {
      toast('Failed to open Settings');
    }
  }

  // Search scoring: name only, hidden entries searchable only when query is typed
  let filteredApps = $derived(() => {
    const query = searchQuery.toLowerCase().trim();
    
    if (!query) {
      // Empty query: show only non-hidden entries
      return availableApps.filter(app => !app.hidden);
    }
    
    // Non-empty query: search ALL entries including hidden ones, sort by score
    return availableApps
      .map(app => ({ app, score: calculateSearchScore(app.name, query) }))
      .filter(({ score }) => score > 0)
      .sort((a, b) => b.score - a.score || a.app.name.localeCompare(b.app.name))
      .map(({ app }) => app);
  });

  function calculateSearchScore(name, query) {
    const nameLower = name.toLowerCase();
    
    // Exact match
    if (nameLower === query) return 100;
    
    // Starts with query
    if (nameLower.startsWith(query)) return 80;
    
    // Word starts with query
    const words = nameLower.split(/\s+/);
    for (const word of words) {
      if (word.startsWith(query)) return 60;
    }
    
    // Contains query
    if (nameLower.includes(query)) return 40;
    
    // Fuzzy subsequence match
    if (isFuzzyMatch(nameLower, query)) return 20;
    
    return 0;
  }

  function isFuzzyMatch(text, query) {
    let queryIdx = 0;
    for (const char of text) {
      if (char === query[queryIdx]) {
        queryIdx++;
        if (queryIdx === query.length) return true;
      }
    }
    return false;
  }

  function highlightMatch(name, query) {
    if (!query) return name;
    const queryLower = query.toLowerCase();
    const nameLower = name.toLowerCase();
    const idx = nameLower.indexOf(queryLower);
    if (idx === -1) return name;
    return name.slice(0, idx) + '<mark>' + name.slice(idx, idx + query.length) + '</mark>' + name.slice(idx + query.length);
  }

  function getHighlightedName(name, query) {
    return highlightMatch(name, query);
  }
</script>

<div class="utility-drawer">
  <div class="section">
    <h3>Windows Tools</h3>
    <div class="tools-grid">
      <button class="tool-card" onclick={openCalc}>
        <Icon name="calc" size={24} />
        <span>Calculator</span>
      </button>
      <button class="tool-card" onclick={openNotepad}>
        <Icon name="file-doc" size={24} />
        <span>Notepad</span>
      </button>
      <button class="tool-card" onclick={openSettings}>
        <Icon name="settings" size={24} />
        <span>Settings</span>
      </button>
    </div>
  </div>

  <div class="section">
    <div class="section-header">
      <h3>My Apps</h3>
      <div class="header-actions">
        <button class="icon-btn" onclick={regenerateIcons} aria-label="Regenerate icons" title="Regenerate icons">
          <Icon name="system" size={14} />
        </button>
        <button class="add-btn" onclick={openAppPicker} aria-label="Add app shortcut">
          <Icon name="plus" size={16} />
        </button>
      </div>
    </div>
    {#if shortcuts.length === 0}
      <div class="empty-state">
        <p>No shortcuts yet. Click + to add apps.</p>
      </div>
    {:else}
      <div class="shortcuts-grid">
        {#each shortcuts as shortcut (shortcut.id)}
          <div
            class="shortcut-card"
            onclick={() => launchShortcut(shortcut)}
            onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') launchShortcut(shortcut); }}
            role="button"
            tabindex="0"
          >
            {#if iconUrls[shortcut.id]}
              <img
                src={iconUrls[shortcut.id]}
                alt={shortcut.name}
                class="shortcut-icon"
                onerror={(e) => { e.target.style.display = 'none'; e.target.nextElementSibling.style.display = 'flex'; }}
              />
            {/if}
            <div class="shortcut-placeholder" style:display={iconUrls[shortcut.id] ? 'none' : 'flex'}>
              <Icon name={getAppIconName(shortcut)} size={24} />
            </div>
            <div class="shortcut-name">{shortcut.name}</div>
            <div
              class="remove-btn"
              onclick={(e) => { e.stopPropagation(); removeShortcut(shortcut); }}
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.stopPropagation(); removeShortcut(shortcut); } }}
              role="button"
              tabindex="0"
              aria-label="Remove"
            >
              <Icon name="trash" size={12} />
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

{#if showAppPicker}
  <div
    class="modal-overlay"
    onclick={(e) => { if (e.target === e.currentTarget) showAppPicker = false; }}
    onkeydown={(e) => { if (e.key === 'Escape') showAppPicker = false; }}
    role="presentation"
  >
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={handleKeydown} role="dialog" aria-label="Add app shortcut" aria-modal="true" tabindex="-1">
      <div class="modal-header">
        <h3>Add App Shortcut</h3>
        <button class="close-btn" onclick={() => showAppPicker = false} aria-label="Close">
          <Icon name="close" size={20} />
        </button>
      </div>
      <div class="search-wrapper">
        <input
          class="search-input"
          bind:value={searchQuery}
          placeholder="Search apps..."
          aria-label="Search apps"
          onfocus={() => this?.select()}
        />
        {#if searchQuery}
          <button class="clear-btn" onclick={() => searchQuery = ''} aria-label="Clear search">
            <Icon name="close" size={14} />
          </button>
        {/if}
      </div>
      <div class="apps-list">
        {#if filteredApps.length === 0}
          <div class="no-results">No apps found "{searchQuery}"</div>
        {:else}
          {#each filteredApps as app, index (app.path)}
            <button
              type="button"
              class="app-item"
              class:highlighted={index === highlightedIndex}
              onclick={() => addShortcut(app)}
              onmouseenter={() => highlightedIndex = index}
              aria-label={`Add ${app.name}`}
            >
              <div class="app-icon">
                {#if appIconUrls[app.path]}
                  <img
                    src={appIconUrls[app.path]}
                    alt={app.name}
                    class="app-icon-img"
                    onerror={(e) => { e.target.style.display = 'none'; e.target.nextElementSibling.style.display = 'flex'; }}
                  />
                {/if}
                <div class="app-icon-placeholder" style:display={appIconUrls[app.path] ? 'none' : 'flex'}>
                  <Icon name={getAppIconName(app)} size={20} />
                </div>
              </div>
              <div class="app-info">
                <div class="app-name" use:innerHTML={getHighlightedName(app.name, searchQuery)}></div>
                {#if app.group}
                  <div class="app-group">{app.group}</div>
                {/if}
              </div>
              <div class="app-action">
                {#if isAppAdded(app)}
                  <Icon name="check" size={16} class="check-icon" />
                {:else}
                  <Icon name="plus" size={16} />
                {/if}
              </div>
            </button>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .utility-drawer {
    padding: 16px;
    height: 100%;
    overflow-y: auto;
  }

  .section {
    margin-bottom: 24px;
  }

  .section h3 {
    margin: 0 0 12px 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--dim);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  /* Tools Grid */
  .tools-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .tool-card {
    background: var(--row);
    border: 1px solid var(--row-hi);
    border-radius: 12px;
    padding: 16px 12px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .tool-card:hover {
    background: var(--row-hi);
    border-color: var(--accent);
    transform: translateY(-2px);
  }

  .tool-card span {
    font-size: 12px;
    color: var(--dim);
  }

  /* App Shortcuts */
  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .section-header h3 {
    margin: 0;
  }

  .header-actions {
    display: flex;
    gap: 8px;
  }

  .icon-btn {
    width: 28px;
    height: 28px;
    border: none;
    background: var(--row-hi);
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: var(--dim);
    transition: background 0.2s;
  }

  .icon-btn:hover {
    background: var(--row-hi);
    color: var(--ink);
  }

  .add-btn {
    width: 28px;
    height: 28px;
    border: none;
    background: var(--row-hi);
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: var(--ink);
    transition: background 0.2s;
  }

  .add-btn:hover {
    background: var(--accent);
  }

  .empty-state {
    text-align: center;
    padding: 24px;
    color: var(--dim);
    font-size: 13px;
  }

  .shortcuts-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .shortcut-card {
    background: var(--row);
    border: 1px solid var(--row-hi);
    border-radius: 10px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    transition: all 0.2s;
    position: relative;
  }

  .shortcut-card:hover {
    background: var(--row-hi);
    border-color: var(--accent);
    transform: translateY(-2px);
  }

  .shortcut-card:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .shortcut-icon {
    width: 48px;
    height: 48px;
    border-radius: 8px;
    object-fit: contain;
  }

  .shortcut-placeholder {
    width: 48px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--row-hi);
    border-radius: 8px;
    color: var(--dim);
  }

  .shortcut-name {
    font-size: 12px;
    font-weight: 500;
    color: var(--ink);
    text-align: center;
    word-break: break-word;
  }

  .remove-btn {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 20px;
    height: 20px;
    border: none;
    background: rgba(255, 138, 128, 0.1);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: #ff8a80;
    opacity: 0;
    transition: opacity 0.2s;
  }

  .shortcut-card:hover .remove-btn {
    opacity: 1;
  }

  .remove-btn:hover {
    background: rgba(255, 138, 128, 0.2);
  }

  .remove-btn:focus-visible {
    opacity: 1;
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* Modal */
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: #14161d;
    border: 1px solid var(--row-hi);
    border-radius: 12px;
    width: 90%;
    max-width: 500px;
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px;
    border-bottom: 1px solid var(--row-hi);
    background: #14161d;
  }

  .modal-header h3 {
    margin: 0;
    font-size: 16px;
  }

  .search-wrapper {
    position: relative;
    padding: 12px 16px;
    border-bottom: 1px solid var(--row-hi);
  }

  .search-input {
    width: 100%;
    padding: 10px 36px 10px 12px;
    border: 1px solid var(--row-hi);
    border-radius: 8px;
    background: var(--row);
    color: var(--ink);
    font-size: 14px;
    outline: none;
  }

  .search-input:focus {
    border-color: var(--accent);
  }

  .clear-btn {
    position: absolute;
    right: 24px;
    top: 50%;
    transform: translateY(-50%);
    width: 24px;
    height: 24px;
    border: none;
    background: transparent;
    color: var(--dim);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
  }

  .clear-btn:hover {
    background: var(--row-hi);
    color: var(--ink);
  }

  .apps-list {
    overflow-y: auto;
    max-height: calc(70vh - 120px);
    padding: 8px;
  }

  .app-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s;
    border: none;
    background: transparent;
    width: 100%;
    text-align: left;
    color: inherit;
    font: inherit;
  }

  .app-item:hover {
    background: var(--row-hi);
  }

  .app-item.highlighted {
    background: var(--row-hi);
    border: 1px solid var(--accent);
  }

  .app-icon {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .app-icon-img {
    width: 32px;
    height: 32px;
    border-radius: 6px;
    object-fit: contain;
  }

  .app-icon-placeholder {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--row-hi);
    border-radius: 6px;
    color: var(--dim);
  }

  .app-info {
    flex: 1;
    min-width: 0;
  }

  .app-name {
    font-size: 14px;
    font-weight: 500;
    color: var(--ink);
    word-break: break-word;
  }

  .app-name :global(mark) {
    background: var(--accent);
    color: white;
    padding: 0 2px;
    border-radius: 2px;
  }

  .app-group {
    font-size: 11px;
    color: var(--dim);
    margin-top: 2px;
  }

  .app-action {
    flex-shrink: 0;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--dim);
  }

  .no-results {
    padding: 32px;
    text-align: center;
    color: var(--dim);
  }

  .close-btn {
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
  }

  .close-btn:hover {
    background: var(--row-hi);
    color: var(--ink);
  }

  .search-input {
    margin: 16px;
    padding: 10px 12px;
    background: var(--row);
    border: 1px solid var(--row-hi);
    border-radius: 8px;
    font-size: 14px;
    color: var(--ink);
  }

  .search-input:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }

  .apps-list {
    flex: 1;
    overflow-y: auto;
    padding: 0 16px 16px;
  }

  .no-results {
    text-align: center;
    padding: 24px;
    color: var(--dim);
    font-size: 13px;
  }

  .app-item {
    width: 100%;
    padding: 12px;
    background: var(--row);
    border: 1px solid var(--row-hi);
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .app-item:hover {
    background: var(--row-hi);
    border-color: var(--accent);
  }

  .app-name {
    font-size: 13px;
    color: var(--ink);
  }
</style>
