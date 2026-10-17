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
      await call('app_shortcut_add', { name: app.name, path: app.path, icon: app.icon });
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
  }

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

  let filteredApps = $derived(availableApps.filter(app => {
    const query = searchQuery.toLowerCase();
    return app.name.toLowerCase().includes(query) ||
           app.path.toLowerCase().includes(query);
  }));
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
    onclick={() => showAppPicker = false}
    onkeydown={(e) => { if (e.key === 'Escape') showAppPicker = false; }}
    role="button"
    tabindex="0"
    aria-label="Close modal"
  >
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <h3>Add App Shortcut</h3>
        <button class="close-btn" onclick={() => showAppPicker = false} aria-label="Close">
          <Icon name="close" size={20} />
        </button>
      </div>
      <input
        class="search-input"
        bind:value={searchQuery}
        placeholder="Search apps..."
        aria-label="Search apps"
      />
      <div class="apps-list">
        {#if filteredApps.length === 0}
          <div class="no-results">No apps found</div>
        {:else}
          {#each filteredApps as app (app.path)}
            <button class="app-item" onclick={() => addShortcut(app)}>
              <span class="app-name">{app.name}</span>
              <Icon name="plus" size={16} />
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
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: var(--bg);
    border-radius: 12px;
    width: 90%;
    max-width: 400px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px;
    border-bottom: 1px solid var(--row-hi);
  }

  .modal-header h3 {
    margin: 0;
    font-size: 16px;
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
