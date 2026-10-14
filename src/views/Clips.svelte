<!-- Clipboard history (text). Backend: clipboard.rs captures, commands.rs serves.
     Click a row to copy it back; pin keeps it forever; clear keeps pinned items. -->
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
</script>

<div class="view">
  <div class="top">
    <input class="grow" placeholder="Search clipboard history" bind:value={q} aria-label="Search clipboard history" />
    <button class="ghost" onclick={clear}>Clear</button>
  </div>
  <div class="scroll">
    {#each items as c (c.id)}
      <div class="row">
        <button class="grow text" onclick={() => copy(c)} title="Copy again">
          <span class="clamp">{c.text}</span>
          <span class="dim small">{ago(c.ts)}</span>
        </button>
        <button class="icon-btn" class:on={c.pinned} class:hover-only={!c.pinned} aria-label={c.pinned ? 'Unpin' : 'Pin'} onclick={() => pin(c)}><Icon name="pin" size={16} /></button>
        <button class="icon-btn hover-only" aria-label="Delete" onclick={() => del(c)}><Icon name="trash" size={16} /></button>
      </div>
    {:else}
      <p class="empty">{q ? 'No matches.' : 'Copy something and it appears here. Passwords from password managers are skipped.'}</p>
    {/each}
  </div>
</div>

<style>
  .top { display: flex; gap: 6px; align-items: center; }
  .text { text-align: left; display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .clamp { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; word-break: break-word; line-height: 1.35; }
  .small { font-size: 11px; }
</style>
