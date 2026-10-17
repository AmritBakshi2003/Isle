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
  let iconUrls = $state({});
  let failedIcons = $state(new Set());  // Track failed icon loads
  let failedAppIcons = $state(new Set());  // Track failed picker icon loads

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

  // Load icons whenever shortcuts change
  $effect(async () => {
    if (shortcuts.length > 0) {
      await loadIconUrls();
    }
  });

  async function loadAvailableApps() {
    try {
      availableApps = await call('enumerate_apps');
    } catch (e) {
      console.error('Failed to enumerate apps:', e);
      toast('Failed to load apps');
      availableApps = [];
    }
  }

  async function addShortcut(app) {
    try {
      await call('app_shortcut_add', {
        name: app.name,
        path: app.path,
        icon: app.icon,
        target: app.target,
        folder_group: app.group || app.folder_group,
        hidden: app.hidden
      });
      toast(`Added ${app.name}`, 2000);
      await loadShortcuts();
      showAppPicker = false;
    } catch (e) {
      console.error('Failed to add shortcut:', e);
      const errorMsg = e?.message || String(e);
      if (errorMsg.includes('is already added')) {
        toast(errorMsg, 3000);
      } else {
        toast(`Failed to add shortcut: ${errorMsg}`, 3000);
      }
    }
  }

  async function removeShortcut(shortcut) {
    try {
      await call('app_shortcut_remove', { id: shortcut.id });
      toast('Removed shortcut', 2000);
      await loadShortcuts();
    } catch (e) {
      console.error('Failed to remove shortcut:', e);
      toast(`Failed to remove shortcut: ${e?.message || String(e)}`, 3000);
    }
  }

  async function launchShortcut(shortcut) {
    try {
      await call('app_shortcut_launch', { path: shortcut.path });
    } catch (e) {
      console.error('Failed to launch shortcut:', e);
      toast('Failed to launch', 3000);
    }
  }

  // Icon loading
  async function loadIconUrls() {
    const iconPromises = [];
    
    for (const shortcut of shortcuts) {
      const promise = (async () => {
        try {
          // If we have icon_filename, try to load it with self-heal
          if (shortcut.icon_filename) {
            const url = await call('app_shortcut_icon', { 
              iconFilename: shortcut.icon_filename,
              id: shortcut.id 
            });
            return { id: shortcut.id, url };
          } else {
            // No icon_filename yet, trigger self-heal by passing empty string
            const url = await call('app_shortcut_icon', { 
              iconFilename: '',
              id: shortcut.id 
            });
            return { id: shortcut.id, url };
          }
        } catch (e) {
          console.error(`[icon] Failed to load icon for ${shortcut.name} (id=${shortcut.id}, icon_filename=${shortcut.icon_filename}):`, e);
          return { id: shortcut.id, url: null };
        }
      })();
      
      iconPromises.push(promise);
    }
    
    // Load all icons in parallel
    const results = await Promise.allSettled(iconPromises);
    const urls = {};
    
    for (const result of results) {
      if (result.status === 'fulfilled' && result.value.url) {
        urls[result.value.id] = result.value.url;
      } else if (result.status === 'fulfilled' && result.value.id) {
        // Mark as failed so we show placeholder
        failedIcons.add(result.value.id);
      }
    }
    
    iconUrls = urls;
  }

  // Calculate Levenshtein distance for typo tolerance
  function levenshteinDistance(a, b) {
    const matrix = [];
    for (let i = 0; i <= b.length; i++) {
      matrix[i] = [i];
    }
    for (let j = 0; j <= a.length; j++) {
      matrix[0][j] = j;
    }
    for (let i = 1; i <= b.length; i++) {
      for (let j = 1; j <= a.length; j++) {
        if (b.charAt(i - 1) === a.charAt(j - 1)) {
          matrix[i][j] = matrix[i - 1][j - 1];
        } else {
          matrix[i][j] = Math.min(
            matrix[i - 1][j - 1] + 1,
            matrix[i][j - 1] + 1,
            matrix[i - 1][j] + 1
          );
        }
      }
    }
    return matrix[b.length][a.length];
  }

  // Search scoring with improved algorithm
  function calculateSearchScore(name, query, folder) {
    const nameLower = name.toLowerCase();
    const folderLower = folder ? folder.toLowerCase() : '';
    
    // Exact match
    if (nameLower === query) return 100;
    
    // Starts with
    if (nameLower.startsWith(query)) return 80;
    
    // Word starts with
    const words = nameLower.split(/\s+/);
    for (const word of words) {
      if (word.startsWith(query)) return 60;
    }
    
    // Folder name match
    if (folderLower && (folderLower.includes(query) || folderLower.startsWith(query))) {
      return 55;
    }
    
    // Contains
    if (nameLower.includes(query)) return 40;
    
    // Typo tolerance (Levenshtein distance)
    const distance = levenshteinDistance(nameLower, query);
    const maxLen = Math.max(nameLower.length, query.length);
    const similarity = 1 - (distance / maxLen);
    if (similarity > 0.7) {
      // Return score based on similarity (70-99% similarity = 25-35 points)
      return Math.floor(similarity * 50);
    }
    
    // Fuzzy subsequence match (fallback)
    let queryIdx = 0;
    for (const char of nameLower) {
      if (char === query[queryIdx]) {
        queryIdx++;
        if (queryIdx === query.length) return 20;
      }
    }
    
    return 0;
  }

  let filteredApps = $derived.by(() => {
    const query = searchQuery.toLowerCase().trim();
    
    if (!query) {
      return availableApps.filter(app => app.hidden !== true);
    }
    
    return availableApps
      .map(app => ({ 
        app, 
        score: calculateSearchScore(app.name, query, app.group || app.folder_group) 
      }))
      .filter(({ score }) => score > 0)
      .sort((a, b) => b.score - a.score || a.app.name.localeCompare(b.app.name))
      .map(({ app }) => app);
  });

  function openAppPicker() {
    loadAvailableApps();
    showAppPicker = true;
    searchQuery = '';
    highlightedIndex = 0;
    failedAppIcons = new Set();  // Reset failed icons for fresh retry
  }

  function highlightMatch(name, query) {
    if (!query) return name;
    const queryLower = query.toLowerCase();
    const nameLower = name.toLowerCase();
    const idx = nameLower.indexOf(queryLower);
    if (idx === -1) return name;
    return name.slice(0, idx) + '<mark>' + name.slice(idx, idx + query.length) + '</mark>' + name.slice(idx + query.length);
  }

  function getAppIconName(app) {
    const name = app.name.toLowerCase();
    // Only return icon names that exist in Icon.svelte
    if (name.includes('chrome') || name.includes('firefox') || name.includes('edge')) return 'media';
    if (name.includes('visual studio') || name.includes('code')) return 'tasks';
    if (name.includes('spotify')) return 'media';
    if (name.includes('discord') || name.includes('slack')) return 'media';
    if (name.includes('notepad') || name.includes('text')) return 'file';
    if (name.includes('explorer') || name.includes('folder')) return 'folder';
    if (name.includes('settings') || name.includes('control') || name.includes('task')) return 'settings';
    if (name.includes('cmd') || name.includes('terminal') || name.includes('power')) return 'console';
    // Default to generic app icon
    return 'app';
  }

  async function loadAppIcon(app) {
    if (appIconUrls[app.path]) {
      console.log(`[picker] Icon already cached for ${app.name}: ${app.path}`);
      return;
    }
    
    try {
      const icon = app.icon || app.path;
      console.log(`[picker] Loading icon for ${app.name} from path: ${icon}`);
      const iconUrl = await call('extract_app_icon', { path: icon });
      if (iconUrl) {
        console.log(`[picker] Successfully loaded icon for ${app.name}: ${iconUrl.substring(0, 50)}...`);
        appIconUrls[app.path] = iconUrl;
      } else {
        console.warn(`[picker] No icon URL returned for ${app.name}`);
        failedAppIcons.add(app.path);
      }
    } catch (e) {
      console.error(`[picker] Failed to load app icon for ${app.name}:`, e);
      failedAppIcons.add(app.path);
    }
  }

  function handleAppIconError(appPath) {
    failedAppIcons.add(appPath);
  }

  // Load icons for visible apps
  $effect(() => {
    if (!showAppPicker) return;
    
    console.log(`[picker] Effect triggered, showAppPicker=true, filteredApps.length=${filteredApps.length}`);
    
    // Load icons for all filtered apps (or first 50 if many results)
    const appsToLoad = filteredApps.slice(0, 50);
    console.log(`[picker] Loading icons for ${appsToLoad.length} apps`);
    for (const app of appsToLoad) {
      loadAppIcon(app);
    }
  });

  function handleIconError(shortcutId) {
    failedIcons.add(shortcutId);
  }

  // Keyboard navigation
  function handleKeyDown(e) {
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
    } else if (e.key === 'Enter' && filteredApps[highlightedIndex]) {
      e.preventDefault();
      addShortcut(filteredApps[highlightedIndex]);
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="utility-drawer">
  <div class="drawer-header">
    <h2>MY APPS</h2>
    <button class="add-btn" onclick={openAppPicker} title="Add App Shortcut">
      <Icon name="plus" size={20} />
    </button>
  </div>

  {#if shortcuts.length === 0}
    <div class="empty-state">
      <Icon name="app" size={48} />
      <p>No shortcuts yet</p>
      <p class="sub">Click + to add apps</p>
    </div>
  {:else}
    <div class="shortcuts-grid">
      {#each shortcuts as shortcut (shortcut.id)}
        <button 
          class="shortcut-card"
          onclick={() => launchShortcut(shortcut)}
          oncontextmenu={(e) => { e.preventDefault(); removeShortcut(shortcut); }}
          title="{shortcut.name} (right-click to remove)"
        >
          <div class="shortcut-icon">
            {#if iconUrls[shortcut.id] && !failedIcons.has(shortcut.id)}
              <img src={iconUrls[shortcut.id]} alt={shortcut.name} onerror={() => handleIconError(shortcut.id)} />
            {:else}
              <Icon name={getAppIconName(shortcut)} size={32} />
            {/if}
          </div>
          <span class="shortcut-name">{shortcut.name}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if showAppPicker}
    <div class="modal-overlay" onclick={(e) => { if (e.target === e.currentTarget) showAppPicker = false; }}>
      <div class="modal" role="dialog" tabindex="-1">
        <div class="modal-header">
          <div class="search-container">
            <Icon name="search" size={16} />
            <input 
              type="text" 
              placeholder="Search apps..." 
              bind:value={searchQuery} 
              use:autofocus
            />
          </div>
          <button class="close-btn" onclick={() => showAppPicker = false}>
            <Icon name="close" size={16} />
          </button>
        </div>

        <div class="apps-list">
          {#if filteredApps.length === 0}
            <div class="no-results">
              <Icon name="search" size={32} />
              <p>No apps found for "{searchQuery}"</p>
            </div>
          {:else}
            {#each filteredApps as app, index (app.path)}
              <button 
                type="button"
                class="app-item"
                class:highlighted={index === highlightedIndex}
                onclick={() => addShortcut(app)}
                onmouseenter={() => highlightedIndex = index}
              >
                <div class="app-icon">
                  {#if appIconUrls[app.path] && !failedAppIcons.has(app.path)}
                    <img src={appIconUrls[app.path]} alt={app.name} onerror={() => handleAppIconError(app.path)} />
                  {:else}
                    <div class="app-icon-placeholder">
                      <Icon name={getAppIconName(app)} size={20} />
                    </div>
                  {/if}
                </div>
                <div class="app-info">
                  <div class="app-name" use:innerHTML={highlightMatch(app.name, searchQuery)}></div>
                  {#if app.group || app.folder_group}
                    <div class="app-group">{app.group || app.folder_group}</div>
                  {/if}
                </div>
                <div class="app-action">
                  {#if isAppAdded(app)}
                    <Icon name="check" size={16} />
                  {:else}
                    <Icon name="plus" size={16} />
                  {/if}
                </div>
              </button>
            {/each}
          {/if}
        </div>

        <div class="modal-footer">
          <span class="keyboard-hint">↑↓ navigate · Enter add · Esc close</span>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .utility-drawer {
    padding: 16px;
  }

  .drawer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }

  .drawer-header h2 {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--dim);
    margin: 0;
  }

  .add-btn {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    border: none;
    background: var(--row);
    color: var(--ink);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.2s;
  }

  .add-btn:hover {
    background: var(--row-hi);
  }

  .empty-state {
    text-align: center;
    padding: 48px 24px;
    color: var(--dim);
  }

  .empty-state p {
    margin: 8px 0 0;
  }

  .empty-state .sub {
    font-size: 13px;
    opacity: 0.7;
  }

  .shortcuts-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
    gap: 12px;
  }

  .shortcut-card {
    background: var(--row);
    border: 1px solid transparent;
    border-radius: 8px;
    padding: 12px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    transition: all 0.2s;
  }

  .shortcut-card:hover {
    background: var(--row-hi);
    border-color: var(--accent);
  }

  .shortcut-icon {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .shortcut-icon img {
    width: 32px;
    height: 32px;
    object-fit: contain;
  }

  .shortcut-name {
    font-size: 11px;
    color: var(--ink);
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    width: 100%;
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
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    width: 480px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .modal-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .search-container {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    padding: 8px 12px;
  }

  .search-container input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--ink);
    font-size: 14px;
  }

  .search-container input::placeholder {
    color: var(--dim);
  }

  .close-btn {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: var(--dim);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--ink);
  }

  .apps-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
    max-height: 400px;
  }

  .app-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px;
    border-radius: 8px;
    border: 1px solid transparent;
    background: transparent;
    cursor: pointer;
    transition: all 0.2s;
    text-align: left;
    width: 100%;
  }

  .app-item:hover,
  .app-item.highlighted {
    background: rgba(255, 255, 255, 0.06);
    border-color: var(--accent);
  }

  .app-icon {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .app-icon img {
    width: 32px;
    height: 32px;
    object-fit: contain;
  }

  .app-icon-placeholder {
    width: 32px;
    height: 32px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .app-info {
    flex: 1;
    min-width: 0;
  }

  .app-name {
    font-size: 14px;
    color: var(--ink);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .app-name mark {
    background: var(--accent);
    color: white;
    padding: 0 2px;
    border-radius: 2px;
  }

  .app-group {
    font-size: 11px;
    color: var(--dim);
    margin-top: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .app-action {
    flex-shrink: 0;
    color: var(--dim);
  }

  .app-item.highlighted .app-action {
    color: var(--accent);
  }

  .no-results {
    text-align: center;
    padding: 32px;
    color: var(--dim);
  }

  .no-results p {
    margin: 12px 0 0;
  }

  .modal-footer {
    padding: 8px 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    justify-content: center;
  }

  .keyboard-hint {
    font-size: 11px;
    color: var(--dim);
  }
</style>
