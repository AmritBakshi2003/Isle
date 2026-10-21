<!-- Music: now playing + transport. State comes from the backend event `media://update`
     (see media.rs); progress is extrapolated here, once per 500 ms, only while this view is mounted.

     Animations are all CSS/Svelte transitions, so nothing ticks while the panel is closed:
       - album art crossfades when the track changes (a new layer fades in over the old one)
       - the title/artist slide up on a track change
       - the progress bar grows with a linear transition between the 500 ms samples
       - the play/pause glyph scales + rotates in place -->
<script>
  import { onMount } from 'svelte';
  // no svelte transition imports: the play/pause glyph swap is an enter-only
  // CSS animation (invariant: never use out: transitions on content swaps —
  // `out:fade` here referenced an unimported `fade`, so the outro threw and
  // the old glyph was never removed, stacking both icons in the button)
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

  // ── album-art crossfade ────────────────────────────────────────────────
  // Two fixed layers, alternating: the hidden one gets the new image, then the
  // pair swaps visibility. Pure CSS opacity transitions do the blend, so no
  // DOM is added or removed and a stalled animation can never leave a stale
  // cover behind (Svelte's outro cleanup would).
  let arts = $state(['', '']); // [layer 0, layer 1]
  let cur = $state(0); // which layer is currently visible
  let lastArt = null;
  $effect(() => {
    const src = media.art ?? '';
    if (!media.active || src === lastArt) return;
    lastArt = src;
    const next = 1 - cur;
    arts[next] = src; // load into the hidden layer…
    cur = next;       // …then swap visibility (CSS fades the swap)
  });

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
        <div class="cover">
          <span class="art-layer" class:show={cur === 0}
                style:background-image={arts[0] ? `url("${arts[0]}")` : 'none'}></span>
          <span class="art-layer" class:show={cur === 1}
                style:background-image={arts[1] ? `url("${arts[1]}")` : 'none'}></span>
          {#if !media.art}<Icon name="music" size={34} />{/if}
        </div>
      {/if}
      <div class="meta grow">
        <div class="title clip">
          {#key media.title}
            <span class="line rise">{media.title || 'Unknown track'}</span>
          {/key}
        </div>
        <div class="dim clip artist">
          {#key media.artist + media.album}
            <span class="line rise slow">{media.artist}{media.album ? ` · ${media.album}` : ''}</span>
          {/key}
        </div>
      </div>
    </div>
    <div class="bar" class:playing={media.playing} role="progressbar" aria-valuemin="0" aria-valuemax={media.duration_ms} aria-valuenow={pos}>
      <span class="fill" style:transform="scaleX({pct})">
        <span class="head"></span>
      </span>
    </div>
    <div class="times dim"><span>{fmtMs(pos)}</span><span>{media.duration_ms ? fmtMs(media.duration_ms) : ''}</span></div>
    <div class="controls">
      {#if mediaSettings.controls === 'full'}
        <button class="icon-btn big" aria-label="Previous" onclick={() => ctl('prev')}><Icon name="prev" size={22} /></button>
      {/if}
      <button class="play" class:paused={!media.playing} aria-label={media.playing ? 'Pause' : 'Play'} onclick={() => ctl('play_pause')}>
        {#key media.playing}
          <span class="glyph pop">
            <Icon name={media.playing ? 'pause' : 'play'} size={22} />
          </span>
        {/key}
      </button>
      {#if mediaSettings.controls === 'full'}
        <button class="icon-btn big" aria-label="Next" onclick={() => ctl('next')}><Icon name="next" size={22} /></button>
      {/if}
    </div>
  {:else}
    <p class="empty">
      Nothing is playing.<br />
      Start music in Spotify, Apple Music, VLC or a browser tab (YouTube, SoundCloud,
      YouTube Music) and it shows up here — those all publish to Windows' media controls.
    </p>
  {/if}
</div>

<style>
  .view { justify-content: center; gap: 8px; }
  .deck { display: flex; gap: 14px; align-items: center; }
  .cover {
    position: relative; overflow: hidden;
    width: 92px; height: 92px; border-radius: 18px; background: var(--row-hi);
    display: grid; place-items: center; color: var(--dim);
    box-shadow: 0 4px 14px color-mix(in srgb, var(--accent) 35%, transparent); flex: none;
  }
  .art-layer {
    position: absolute; inset: 0; background: center/cover no-repeat;
    opacity: 0; transition: opacity .38s ease; will-change: opacity;
  }
  .art-layer.show { opacity: 1; }
  .title { font-size: 17px; font-weight: 650; }
  .artist { font-size: 12.5px; }
  .meta { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .line { display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* enter-only animation: the previous line is removed immediately, so a stalled
     animation can never leave two titles stacked in the same row */
  .rise { animation: rise .3s cubic-bezier(.22,.9,.3,1) both; }
  .rise.slow { animation-duration: .36s; }
  @keyframes rise {
    from { opacity: 0; transform: translateY(8px); }
    to   { opacity: 1; transform: none; }
  }
  .bar { position: relative; height: 5px; border-radius: 3px; background: var(--row-hi); overflow: hidden; margin-top: 6px; }
  .fill {
    display: block; height: 100%; width: 100%; border-radius: 3px; background: var(--accent);
    transform-origin: left; transition: transform .55s linear;
    will-change: transform;
  }
  .bar.playing .fill { box-shadow: 0 0 10px color-mix(in srgb, var(--accent) 65%, transparent); }
  .head {
    position: absolute; right: 0; top: 50%; width: 5px; height: 5px; margin-top: -2.5px;
    border-radius: 50%; background: #fff; opacity: 0; transition: opacity .25s ease;
  }
  .bar.playing .head { opacity: .85; }
  .times { display: flex; justify-content: space-between; font-size: 11px; font-variant-numeric: tabular-nums; }
  .controls { display: flex; justify-content: center; align-items: center; gap: 22px; }
  .big { width: 40px; height: 40px; border-radius: 50%; color: var(--ink); display: grid; place-items: center;
    transition: background .18s ease, color .18s ease, transform .18s var(--spring); }
  .big:hover { background: var(--row-hi); }
  .big:active { transform: scale(.9); }
  .play {
    position: relative; width: 50px; height: 50px; border-radius: 50%;
    background: var(--ink); color: #0a0a0a; display: grid; place-items: center;
    transition: transform .22s var(--spring), background .3s ease;
  }
  .play:hover { transform: scale(1.06); }
  .play:active { transform: scale(.92); }
  .glyph { display: grid; place-items: center; }
  /* enter-only: {#key} swaps the node synchronously, so exactly one glyph is
     ever in the DOM; the animation replays on each remount (scale + rotate) */
  .glyph.pop { animation: glyph-pop .22s cubic-bezier(.22,.9,.3,1) both; }
  @keyframes glyph-pop {
    from { opacity: 0; transform: scale(.55) rotate(-25deg); }
    to   { opacity: 1; transform: none; }
  }
</style>
