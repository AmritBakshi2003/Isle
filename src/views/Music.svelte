<!-- Music: now playing + transport. State comes from the backend event `media://update`
     (see media.rs); progress is extrapolated here, once per 500 ms, only while this view is mounted. -->
<script>
  import { onMount } from 'svelte';
  import { call } from '../lib/api.js';
  import { media, livePosition, toast } from '../lib/state.svelte.js';
  import { fmtMs } from '../lib/util.js';
  import Icon from '../lib/Icon.svelte';

  let pos = $state(livePosition());
  const pct = $derived(media.duration_ms > 0 ? Math.min(1, pos / media.duration_ms) : 0);

  let mediaSettings = $state({
    album_art: 'true',
    controls: 'full',
    update_speed: '1000',
  });

  const updateInterval = $derived(Number(mediaSettings.update_speed));

  onMount(async () => {
    // Load media settings
    try {
      const all = await call('settings_get_all');
      mediaSettings.album_art = all.media_album_art ?? 'true';
      mediaSettings.controls = all.media_controls ?? 'full';
      mediaSettings.update_speed = all.media_update_speed ?? '1000';
    } catch (e) { console.warn('Failed to load media settings:', e); }

    const id = setInterval(() => (pos = livePosition()), updateInterval);
    return () => clearInterval(id);
  });

  $effect(() => { media.sampled_at_ms; media.playing; pos = livePosition(); });

  const ctl = (cmd) => call('media_control', { cmd }).catch((e) => toast(e));
</script>

<div class="view">
  {#if media.active}
    <div class="deck">
      {#if mediaSettings.album_art === 'true'}
        <div class="cover" style:background-image={media.art ? `url(${media.art})` : 'none'}>
          {#if !media.art}<Icon name="music" size={34} />{/if}
        </div>
      {/if}
      <div class="meta grow">
        <div class="title clip">{media.title || 'Unknown track'}</div>
        <div class="dim clip">{media.artist}{media.album ? ` · ${media.album}` : ''}</div>
      </div>
    </div>
    <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax={media.duration_ms} aria-valuenow={pos}>
      <span class="fill" style:transform="scaleX({pct})"></span>
    </div>
    <div class="times dim"><span>{fmtMs(pos)}</span><span>{media.duration_ms ? fmtMs(media.duration_ms) : ''}</span></div>
    <div class="controls">
      {#if mediaSettings.controls === 'full'}
        <button class="icon-btn big" aria-label="Previous" onclick={() => ctl('prev')}><Icon name="prev" size={22} /></button>
      {/if}
      <button class="play" aria-label={media.playing ? 'Pause' : 'Play'} onclick={() => ctl('play_pause')}>
        <Icon name={media.playing ? 'pause' : 'play'} size={22} />
      </button>
      {#if mediaSettings.controls === 'full'}
        <button class="icon-btn big" aria-label="Next" onclick={() => ctl('next')}><Icon name="next" size={22} /></button>
      {/if}
    </div>
  {:else}
    <p class="empty">Nothing is playing.<br />Start music in Spotify, your browser or any media app and it shows up here.</p>
  {/if}
</div>

<style>
  .view { justify-content: center; gap: 8px; }
  .deck { display: flex; gap: 14px; align-items: center; }
  .cover { width: 92px; height: 92px; border-radius: 18px; background: var(--row-hi) center/cover; display: grid; place-items: center; color: var(--dim);
    box-shadow: 0 4px 14px color-mix(in srgb, var(--accent) 35%, transparent); flex: none; }
  .title { font-size: 17px; font-weight: 650; }
  .meta { display: flex; flex-direction: column; gap: 3px; }
  .bar { height: 5px; border-radius: 3px; background: var(--row-hi); overflow: hidden; margin-top: 6px; }
  .fill { display: block; height: 100%; width: 100%; background: var(--accent); transform-origin: left; transition: transform .5s linear; }
  .times { display: flex; justify-content: space-between; font-size: 11px; font-variant-numeric: tabular-nums; }
  .controls { display: flex; justify-content: center; align-items: center; gap: 22px; }
  .big { width: 40px; height: 40px; border-radius: 50%; color: var(--ink); }
  .play { width: 50px; height: 50px; border-radius: 50%; background: var(--ink); color: #0a0a0a; display: grid; place-items: center; transition: transform .2s var(--spring); }
  .play:active { transform: scale(.92); }
</style>
