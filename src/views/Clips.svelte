<!-- Clipboard history (text + files). Backend: clipboard.rs captures, commands.rs serves.
     Click a row to copy it back; pin keeps it forever; clear keeps pinned items.
     Files can be dragged onto the pill to store them. -->
<script>
  import { onMount } from 'svelte';
  import { call, on } from '../lib/api.js';
  import { toast } from '../lib/state.svelte.js';
  import { ago } from '../lib/util.js';
  import Icon from '../lib/Icon.svelte';

  let q = $state('');
  let items = $state([]);
  const load = async () => (items = await call('clip_list', { query: q || null }));

  $effect(() => { q; load(); });
  onMount(() => on('clipboard://changed', load));

  async function copy(c) {
    try { await call('clip_copy', { id: c.id }); toast('Copied'); } catch (e) { toast(e); }
  }
  const pin = async (c) => { await call('clip_pin', { id: c.id }); load(); };
  const del = async (c) => { await call('clip_delete', { id: c.id }); load(); };
  const clear = async () => { await call('clip_clear'); load(); toast('Cleared. Pinned items kept'); };

  async function openFile(c) {
    try { await call('clip_open_file', { path: c.file_path }); toast('Opened'); } catch (e) { toast(e); }
  }
  async function deleteFile(c) {
    try { await call('clip_delete_file', { path: c.file_path }); load(); toast('File removed'); } catch (e) { toast(e); }
  }

  function isFile(c) {
    return c.file_path !== null && c.file_path !== undefined;
  }

  function getFileIcon(c) {
    const path = c.file_path?.toLowerCase() || '';
    const ext = c.file_type?.toLowerCase() || '';

    // Check if it's a folder (no extension or path ends with separator)
    if (!ext || path.endsWith('\\') || path.endsWith('/')) {
      return 'folder';
    }

    if (ext === 'pdf') return 'file-pdf';
    if (ext === 'doc' || ext === 'docx') return 'file-doc';
    if (ext === 'xls' || ext === 'xlsx' || ext === 'csv') return 'file-xls';
    if (ext === 'jpg' || ext === 'jpeg' || ext === 'png' || ext === 'gif' || ext === 'webp' || ext === 'svg') return 'file-image';
    if (ext === 'mp4' || ext === 'avi' || ext === 'mkv' || ext === 'mov' || ext === 'wmv') return 'file-video';
    if (ext === 'js' || ext === 'ts' || ext === 'py' || ext === 'java' || ext === 'cpp' || ext === 'c' || ext === 'html' || ext === 'css' || ext === 'json' || ext === 'sql') return 'file-code';
    if (ext === 'zip' || ext === 'rar' || ext === '7z' || ext === 'tar' || ext === 'gz') return 'file-zip';
    return 'file';
  }

  function handleDragStart(e, c) {
    if (isFile(c)) {
      e.dataTransfer.setData('text/uri-list', c.file_path);
      e.dataTransfer.setData('text/plain', c.file_path);
      e.dataTransfer.effectAllowed = 'copy';
    }
  }

  const textItems = $derived(items.filter(c => !isFile(c)));
  const fileItems = $derived(items.filter(c => isFile(c)));
</script>

<div class="view">
  <div class="top">
    <input class="grow" placeholder="Search clipboard history" bind:value={q} aria-label="Search clipboard history" />
    <button class="ghost" onclick={clear}>Clear</button>
  </div>
  <div class="columns">
    <div class="column">
      <div class="column-header">Text</div>
      <div class="scroll">
        {#each textItems as c (c.id)}
          <div class="row">
            <button class="grow text" onclick={() => copy(c)} title="Copy again">
              <span class="clamp">{c.text}</span>
              <span class="dim small">{ago(c.ts)}</span>
            </button>
            <button class="icon-btn" class:on={c.pinned} class:hover-only={!c.pinned} aria-label={c.pinned ? 'Unpin' : 'Pin'} onclick={() => pin(c)}><Icon name="pin" size={16} /></button>
            <button class="icon-btn hover-only" aria-label="Delete" onclick={() => del(c)}><Icon name="trash" size={16} /></button>
          </div>
        {:else}
          <p class="empty">No text clips.</p>
        {/each}
      </div>
    </div>
    <div class="column">
      <div class="column-header">Files</div>
      <div class="scroll">
        {#each fileItems as c (c.id)}
          <div class="row file-row" draggable ondragstart={(e) => handleDragStart(e, c)}>
            <button class="grow text" onclick={() => openFile(c)} title="Open file">
              <span class="clamp file-name"><Icon name={getFileIcon(c)} size={14} /> {c.file_name}</span>
              <span class="dim small">{ago(c.ts)}</span>
            </button>
            <button class="icon-btn hover-only" aria-label="Open" onclick={() => openFile(c)}><Icon name="open" size={16} /></button>
            <button class="icon-btn hover-only" aria-label="Delete" onclick={() => deleteFile(c)}><Icon name="trash" size={16} /></button>
          </div>
        {:else}
          <p class="empty">No files. Drag files here to add them.</p>
        {/each}
      </div>
    </div>
  </div>
</div>

<style>
  .top { display: flex; gap: 6px; align-items: center; }
  .columns { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; height: calc(100% - 44px); overflow: hidden; }
  .column { display: flex; flex-direction: column; gap: 8px; height: 100%; overflow: hidden; }
  .column-header { font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: rgba(255,255,255,0.5); padding: 0 4px; flex-shrink: 0; }
  .scroll { flex: 1; overflow-y: auto; overflow-x: hidden; min-height: 0; padding-right: 4px; }
  .scroll::-webkit-scrollbar { width: 6px; }
  .scroll::-webkit-scrollbar-track { background: transparent; }
  .scroll::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.2); border-radius: 3px; }
  .scroll::-webkit-scrollbar-thumb:hover { background: rgba(255,255,255,0.3); }
  .text { text-align: left; display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .clamp { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; word-break: break-word; line-height: 1.35; }
  .small { font-size: 11px; }
  .file-name { display: flex; align-items: center; gap: 6px; }
  .file-row { background: rgba(255, 255, 255, 0.03); }
  .empty { font-size: 12px; color: rgba(255,255,255,0.4); text-align: center; padding: 20px 0; }
</style>
