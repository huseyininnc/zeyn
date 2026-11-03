/**
 * Zeyn media players.
 *
 * Browser behavior of `zeyn::ui::Video` and `zeyn::ui::Audio`: the native `<video>` or `<audio>`
 * element plays, and the custom controls drive it. Sliders follow the WAI-ARIA slider pattern,
 * menus the menu button pattern. Single key shortcuts (k, j, l, m, c, f) work only while focus is
 * inside the player. Files come from `/_zeyn/media/{source}/{id}`, which checks access and answers
 * range requests, so seeking only fetches what is needed.
 *
 * Adaptive streaming: HLS plays natively where the browser supports it (Safari). Elsewhere the
 * player dispatches a cancelable `zeyn:media-adaptive` event on its root with
 * `{media, kind, url, hls, dash}`; a page script that attaches a streaming library calls
 * `preventDefault()` to take over. Otherwise the progressive files play.
 *
 * Resume: the position is read from and saved to `/_zeyn/media/position` when the media source
 * keeps positions for the signed in user; otherwise it is kept in this browser's local storage.
 *
 * Embeds: `zeyn::ui::Embed` with `click_to_load` renders a poster button and the frame inside an
 * inert `<template>`. Pressing the button puts the frame in its place and moves focus to it. With
 * `remember` the end time of the consent is kept in `localStorage`, and later pages and tabs load
 * the frames of that scope by themselves without moving focus.
 * @module media
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const SAVE_EVERY_MS = 15000;
const RESUME_MARGIN_S = 5;
const SEEK_STEP_S = 5;
const JUMP_STEP_S = 10;
const VOLUME_STEP = 5;
/** Prefix of the `localStorage` keys that hold the end time of a remembered embed consent. */
const CONSENT = 'zeyn.consent.';

/** Player state by root element. */
const players = new WeakMap();

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/** Parses JSON, or returns `null`. */
function parse(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** Fills `{name}` placeholders. */
function fill(template, values) {
  return String(template ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in values ? values[k] : m));
}

/**
 * Seconds as `m:ss` or `h:mm:ss`.
 * @example clock(75) // "1:15"
 */
function clock(seconds) {
  const s = Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0;
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, '0');
  return h ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** Local storage access that never throws (private windows, blocked storage). */
const local = {
  get(key) {
    try {
      return parse(window.localStorage.getItem(key) ?? 'null');
    } catch {
      return null;
    }
  },
  set(key, value) {
    try {
      window.localStorage.setItem(key, JSON.stringify(value));
    } catch {
      /* Storage is unavailable; the position is simply not kept. */
    }
  },
};

/** Sets up every player and remembered embed inside `root`. Safe to call more than once. */
function init(root) {
  root.querySelectorAll('[data-zeyn-player]').forEach(setup);
  restoreEmbeds(root);
}

/** Wires one player: sources, tracks, events and the saved position. */
function setup(root) {
  if (players.has(root)) return;
  const media = root.querySelector('.z-player-media');
  if (!media) return;
  const p = {
    root,
    media,
    texts: parse(root.dataset.zeynTexts ?? '{}') ?? {},
    source: root.dataset.zeynMediaSource ?? '',
    id: root.dataset.zeynMediaId ?? '',
    server: false,
    savedAt: 0,
    track: preferredTrack(media),
    seeking: false,
  };
  players.set(root, p);
  if (media.dataset.zeynPoster) media.poster = BASE + media.dataset.zeynPoster;
  media.querySelectorAll('track[data-zeyn-src]').forEach((t) => {
    t.src = BASE + t.dataset.zeynSrc;
  });
  for (const t of media.textTracks) t.mode = 'disabled';
  for (const t of media.textTracks) t.addEventListener('cuechange', () => showCues(p));
  const on = (type, fn, options) => media.addEventListener(type, fn, options);
  on('play', () => paint(p));
  on('pause', () => {
    paint(p);
    save(p, media.ended ? 0 : media.currentTime);
  });
  on('ended', () => save(p, 0));
  on('timeupdate', () => {
    paintTime(p);
    if (!media.paused && Date.now() - p.savedAt > SAVE_EVERY_MS) save(p, media.currentTime);
  });
  on('durationchange', () => paintTime(p));
  on('progress', () => paintTime(p));
  on('volumechange', () => paintVolume(p));
  on('loadedmetadata', () => resume(p), { once: true });
  on('enterpictureinpicture', () => paintPip(p, true));
  on('leavepictureinpicture', () => paintPip(p, false));
  media.addEventListener(
    'error',
    () => {
      if (media.error || media.networkState === HTMLMediaElement.NETWORK_NO_SOURCE) {
        root.querySelector('[data-zeyn-player-error]')?.removeAttribute('hidden');
      }
    },
    true,
  );
  const pip = root.querySelector('[data-zeyn-player-pip]');
  if (pip && document.pictureInPictureEnabled && !media.disablePictureInPicture) pip.hidden = false;
  load(p);
  paint(p);
  paintVolume(p);
  paintTime(p);
}

/** Index of the captions track in the page's language, or the first one. */
function preferredTrack(media) {
  const lang = (document.documentElement.lang || 'en').slice(0, 2).toLowerCase();
  const tracks = [...media.querySelectorAll('track')];
  const i = tracks.findIndex((t) => (t.srclang || '').slice(0, 2).toLowerCase() === lang);
  return i >= 0 ? i : 0;
}

/**
 * Chooses what to play: native HLS, an adaptive library through `zeyn:media-adaptive`, or the
 * progressive files in order.
 */
function load(p) {
  const { media, root } = p;
  const hls = root.dataset.zeynHls ? BASE + root.dataset.zeynHls : null;
  const dash = root.dataset.zeynDash ? BASE + root.dataset.zeynDash : null;
  if (hls && media.canPlayType('application/vnd.apple.mpegurl')) {
    media.src = hls;
    return;
  }
  if (hls || dash) {
    const detail = { media, kind: hls ? 'hls' : 'dash', url: hls ?? dash, hls, dash };
    const taken = !root.dispatchEvent(new CustomEvent('zeyn:media-adaptive', { bubbles: true, cancelable: true, detail }));
    if (taken) return;
  }
  media.querySelectorAll('source[data-zeyn-src]').forEach((s) => {
    s.src = BASE + s.dataset.zeynSrc;
  });
  media.load();
}

/* Painting ----------------------------------------------------------------------------------- */

function paint(p) {
  const playing = !p.media.paused;
  p.root.classList.toggle('z-playing', playing);
  p.root.querySelectorAll('.z-player-controls [data-zeyn-player-play]').forEach((b) => {
    b.setAttribute('aria-label', playing ? p.texts.pause : p.texts.play);
  });
}

function paintTime(p) {
  const { media, root } = p;
  const duration = Number.isFinite(media.duration) ? media.duration : 0;
  const now = media.currentTime;
  const seek = root.querySelector('[data-zeyn-player-seek]');
  if (seek && !p.seeking) {
    seek.setAttribute('aria-valuemax', String(Math.floor(duration)));
    seek.setAttribute('aria-valuenow', String(Math.floor(now)));
    seek.setAttribute('aria-valuetext', fill(p.texts.time_of, { current: clock(now), total: clock(duration) }));
    seek.style.setProperty('--z-at', duration ? String(now / duration) : '0');
  }
  if (seek && duration && media.buffered.length) {
    seek.style.setProperty('--z-buffered', String(media.buffered.end(media.buffered.length - 1) / duration));
  }
  const time = root.querySelector('[data-zeyn-player-time]');
  if (time) time.textContent = `${clock(now)} / ${clock(duration)}`;
}

function paintVolume(p) {
  const { media, root } = p;
  const muted = media.muted || media.volume === 0;
  root.classList.toggle('z-muted', muted);
  const mute = root.querySelector('[data-zeyn-player-mute]');
  mute?.setAttribute('aria-pressed', String(media.muted));
  mute?.setAttribute('aria-label', media.muted ? p.texts.unmute : p.texts.mute);
  const slider = root.querySelector('[data-zeyn-player-volume]');
  if (!slider) return;
  const v = Math.round((media.muted ? 0 : media.volume) * 100);
  slider.setAttribute('aria-valuenow', String(v));
  slider.setAttribute('aria-valuetext', `${v}%`);
  slider.style.setProperty('--z-at', String(v / 100));
}

function paintPip(p, on) {
  p.root.querySelector('[data-zeyn-player-pip]')?.setAttribute('aria-pressed', String(on));
}

/** Says something to screen reader users through the player's status region. */
function announce(p, text) {
  const status = p.root.querySelector('[data-zeyn-player-status]');
  if (status) status.textContent = text;
}

/* Actions ------------------------------------------------------------------------------------ */

function toggle(p) {
  if (p.media.paused) p.media.play().catch(() => {});
  else p.media.pause();
}

function seekTo(p, seconds) {
  const duration = Number.isFinite(p.media.duration) ? p.media.duration : 0;
  if (!duration) return;
  p.media.currentTime = Math.min(Math.max(seconds, 0), duration);
  paintTime(p);
}

function setVolume(p, percent) {
  const v = Math.min(Math.max(percent, 0), 100) / 100;
  p.media.volume = v;
  p.media.muted = v === 0;
}

function setSpeed(p, rate) {
  p.media.playbackRate = rate;
  const button = p.root.querySelector('[data-zeyn-player-speed]');
  if (button) {
    button.textContent = `${rate}×`;
    button.setAttribute('aria-label', fill(p.texts.speed_value, { value: `${rate}×` }));
  }
  p.root.querySelectorAll('[data-zeyn-speed]').forEach((li) => li.setAttribute('aria-checked', String(Number(li.dataset.zeynSpeed) === rate)));
}

/** Shows captions track `index`, or hides captions with `-1`. */
function setCaptions(p, index) {
  const tracks = [...p.media.textTracks];
  tracks.forEach((t, i) => {
    t.mode = i === index ? 'hidden' : 'disabled';
  });
  const on = index >= 0 && index < tracks.length;
  if (on) p.track = index;
  p.captionsOn = on;
  const cc = p.root.querySelector('[data-zeyn-player-cc]');
  cc?.setAttribute('aria-pressed', String(on));
  p.root.querySelectorAll('[data-zeyn-track]').forEach((li) => li.setAttribute('aria-checked', String(Number(li.dataset.zeynTrack) === (on ? index : -1))));
  const area = p.root.querySelector('[data-zeyn-player-captions]');
  if (area) area.hidden = !on;
  showCues(p);
  announce(p, on ? fill(p.texts.captions_on, { label: tracks[index].label }) : p.texts.captions_off);
}

/** Writes the active cues as plain text; cue markup is dropped, never parsed as HTML. */
function showCues(p) {
  const area = p.root.querySelector('[data-zeyn-player-captions]');
  if (!area) return;
  const track = p.captionsOn ? p.media.textTracks[p.track] : null;
  const cues = track?.activeCues ? [...track.activeCues] : [];
  area.textContent = cues.map((c) => String(c.text ?? '').replace(/<[^>]*>/g, '')).join('\n');
  area.classList.toggle('z-empty', !area.textContent);
}

function fullscreen(p) {
  const { root, media } = p;
  if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
  else if (root.requestFullscreen) root.requestFullscreen().catch(() => {});
  else media.webkitEnterFullscreen?.();
}

function pip(p) {
  if (document.pictureInPictureElement) document.exitPictureInPicture().catch(() => {});
  else p.media.requestPictureInPicture?.().catch(() => {});
}

/* Resume ------------------------------------------------------------------------------------- */

function localKey(p) {
  return `zeyn.media.${p.source}/${p.id}`;
}

/** Jumps to where the user stopped, from the server when the source keeps positions. */
async function resume(p) {
  if (!p.id) return;
  let at = null;
  try {
    const q = `?source=${encodeURIComponent(p.source)}&id=${encodeURIComponent(p.id)}`;
    const res = await fetch(`${BASE}/_zeyn/media/position${q}`, { credentials: 'same-origin', headers: { accept: 'application/json' } });
    if (res.ok) {
      const body = await res.json();
      p.server = body?.kept === true;
      at = body?.at ?? null;
    }
  } catch {
    p.server = false;
  }
  if (!p.server) at = local.get(localKey(p));
  const duration = p.media.duration;
  if (typeof at === 'number' && at > RESUME_MARGIN_S && Number.isFinite(duration) && at < duration - RESUME_MARGIN_S && p.media.currentTime < 1) {
    p.media.currentTime = at;
    announce(p, fill(p.texts.resumed, { time: clock(at) }));
  }
}

/** Saves the position; `keepalive` lets the request finish while the page unloads. */
function save(p, at, keepalive = false) {
  if (!p.id || !Number.isFinite(at)) return;
  p.savedAt = Date.now();
  const seconds = Math.round(at * 10) / 10;
  if (!p.server) {
    local.set(localKey(p), seconds);
    return;
  }
  csrf()
    .then((token) =>
      fetch(`${BASE}/_zeyn/media/position`, {
        method: 'POST',
        credentials: 'same-origin',
        keepalive,
        headers: { 'content-type': 'application/json', 'x-zeyn-csrf': token },
        body: JSON.stringify({ source: p.source, id: p.id, at: seconds }),
      }),
    )
    .catch(() => {});
}

/** Saves every playing player before the page goes away or is hidden. */
function saveAll() {
  document.querySelectorAll('[data-zeyn-player]').forEach((root) => {
    const p = players.get(root);
    if (p && !p.media.paused) save(p, p.media.currentTime, true);
  });
}

/* Sliders ------------------------------------------------------------------------------------ */

/** Fraction of a slider at a pointer position, from 0 to 1, right to left aware. */
function fractionAt(slider, clientX) {
  const rail = slider.querySelector('.z-player-rail') ?? slider;
  const box = rail.getBoundingClientRect();
  const f = box.width ? (clientX - box.left) / box.width : 0;
  const rtl = getComputedStyle(slider).direction === 'rtl';
  return Math.min(Math.max(rtl ? 1 - f : f, 0), 1);
}

/** Drags a slider with a pointer; seeking commits on release so only one range request is made. */
function dragSlider(e, slider, p) {
  e.preventDefault();
  slider.focus();
  const isSeek = slider.hasAttribute('data-zeyn-player-seek');
  const duration = Number.isFinite(p.media.duration) ? p.media.duration : 0;
  const move = (ev) => {
    const f = fractionAt(slider, ev.clientX);
    if (isSeek) {
      p.seeking = true;
      slider.style.setProperty('--z-at', String(f));
      slider.setAttribute('aria-valuenow', String(Math.floor(f * duration)));
      slider.setAttribute('aria-valuetext', fill(p.texts.time_of, { current: clock(f * duration), total: clock(duration) }));
    } else setVolume(p, f * 100);
    return f;
  };
  let last = move(e);
  slider.setPointerCapture?.(e.pointerId);
  const up = (ev) => {
    last = move(ev);
    slider.removeEventListener('pointermove', onMove);
    slider.removeEventListener('pointerup', up);
    slider.removeEventListener('pointercancel', up);
    if (isSeek) {
      p.seeking = false;
      seekTo(p, last * duration);
    }
  };
  const onMove = (ev) => {
    last = move(ev);
  };
  slider.addEventListener('pointermove', onMove);
  slider.addEventListener('pointerup', up);
  slider.addEventListener('pointercancel', up);
}

/** Arrow, Page, Home and End keys on the seek and volume sliders. */
function sliderKey(e, slider, p) {
  const seek = slider.hasAttribute('data-zeyn-player-seek');
  const duration = Number.isFinite(p.media.duration) ? p.media.duration : 0;
  const now = seek ? p.media.currentTime : Math.round((p.media.muted ? 0 : p.media.volume) * 100);
  const step = seek ? SEEK_STEP_S : VOLUME_STEP;
  const big = seek ? Math.max(duration / 10, JUMP_STEP_S) : VOLUME_STEP * 4;
  const max = seek ? duration : 100;
  const to = {
    ArrowRight: now + step,
    ArrowUp: now + step,
    ArrowLeft: now - step,
    ArrowDown: now - step,
    PageUp: now + big,
    PageDown: now - big,
    Home: 0,
    End: max,
  }[e.key];
  if (to === undefined) return false;
  e.preventDefault();
  if (seek) seekTo(p, to);
  else setVolume(p, to);
  return true;
}

/* Menus -------------------------------------------------------------------------------------- */

let openMenu = null;

function showMenu(button) {
  closeMenu(false);
  const list = document.getElementById(button.getAttribute('aria-controls'));
  if (!list) return;
  list.hidden = false;
  button.setAttribute('aria-expanded', 'true');
  openMenu = { button, list };
  const items = [...list.querySelectorAll('[role="menuitemradio"]')];
  (items.find((i) => i.getAttribute('aria-checked') === 'true') ?? items[0])?.focus();
}

function closeMenu(focusButton) {
  if (!openMenu) return;
  const { button, list } = openMenu;
  openMenu = null;
  list.hidden = true;
  button.setAttribute('aria-expanded', 'false');
  if (focusButton) button.focus();
}

function chooseItem(item, p) {
  if (item.dataset.zeynSpeed) setSpeed(p, Number(item.dataset.zeynSpeed));
  else if (item.dataset.zeynTrack !== undefined) setCaptions(p, Number(item.dataset.zeynTrack));
  closeMenu(true);
}

function menuKey(e, item, p) {
  const items = [...item.closest('[role="menu"]').querySelectorAll('[role="menuitemradio"]')];
  const i = items.indexOf(item);
  const go = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: items.length - 1 }[e.key];
  if (go !== undefined) {
    e.preventDefault();
    items[(go + items.length) % items.length].focus();
  } else if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault();
    chooseItem(item, p);
  } else if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    closeMenu(true);
  } else if (e.key === 'Tab') closeMenu(false);
  return true;
}

/* Events ------------------------------------------------------------------------------------- */

/** The player and its state for an event target. */
function playerOf(t) {
  const root = t instanceof Element ? t.closest('[data-zeyn-player]') : null;
  const p = root ? players.get(root) : null;
  return p ? [root, p] : [null, null];
}

document.addEventListener('click', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  const [, p] = playerOf(t);
  if (!p) {
    if (openMenu) closeMenu(false);
    return;
  }
  const item = t.closest('[role="menuitemradio"]');
  if (item) return chooseItem(item, p);
  const menu = t.closest('[data-zeyn-player-menu]');
  if (menu) return menu.getAttribute('aria-expanded') === 'true' ? closeMenu(true) : showMenu(menu);
  if (openMenu) closeMenu(false);
  if (t.closest('[data-zeyn-player-play]') || t.closest('.z-player-stage')) return toggle(p);
  if (t.closest('[data-zeyn-player-mute]')) {
    p.media.muted = !p.media.muted;
    if (!p.media.muted && p.media.volume === 0) p.media.volume = 1;
    return;
  }
  if (t.closest('[data-zeyn-player-cc]')) return setCaptions(p, p.captionsOn ? -1 : p.track);
  if (t.closest('[data-zeyn-player-fullscreen]')) return fullscreen(p);
  if (t.closest('[data-zeyn-player-pip]')) return pip(p);
});

document.addEventListener('dblclick', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  const [, p] = playerOf(t);
  if (p && t.closest('.z-player-video .z-player-stage')) fullscreen(p);
});

document.addEventListener('pointerdown', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  const slider = t?.closest('[data-zeyn-player] .z-player-slider');
  if (!slider || e.button !== 0) return;
  const [, p] = playerOf(slider);
  if (p) dragSlider(e, slider, p);
});

document.addEventListener('keydown', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  const [, p] = playerOf(t);
  if (!p || e.ctrlKey || e.altKey || e.metaKey) return;
  const item = t.closest('[role="menuitemradio"]');
  if (item) return menuKey(e, item, p);
  const slider = t.closest('.z-player-slider');
  if (slider && sliderKey(e, slider, p)) return;
  if (t.closest('button') && (e.key === ' ' || e.key === 'Enter')) return;
  const key = (e.key ?? '').toLowerCase();
  const actions = {
    ' ': () => toggle(p),
    k: () => toggle(p),
    j: () => seekTo(p, p.media.currentTime - JUMP_STEP_S),
    l: () => seekTo(p, p.media.currentTime + JUMP_STEP_S),
    m: () => {
      p.media.muted = !p.media.muted;
    },
    c: () => p.root.querySelector('[data-zeyn-player-cc]') && setCaptions(p, p.captionsOn ? -1 : p.track),
    f: () => p.root.querySelector('[data-zeyn-player-fullscreen]') && fullscreen(p),
  };
  if (!actions[key] || t.matches('input, textarea, select, [contenteditable="true"]')) return;
  e.preventDefault();
  actions[key]();
});

document.addEventListener('fullscreenchange', () => {
  document.querySelectorAll('[data-zeyn-player-fullscreen]').forEach((b) => {
    const [root, p] = playerOf(b);
    const on = document.fullscreenElement === root;
    root.classList.toggle('z-fullscreen', on);
    b.setAttribute('aria-label', on ? p.texts.exit_fullscreen : p.texts.fullscreen);
  });
});

window.addEventListener('pagehide', saveAll);
document.addEventListener('visibilitychange', () => document.visibilityState === 'hidden' && saveAll());

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));

new MutationObserver((records) => {
  for (const r of records) for (const n of r.addedNodes) if (n instanceof HTMLDialogElement) init(n);
}).observe(document.body, { childList: true });

init(document);

/**
 * Whether the embeds of `e`'s scope hold a consent that has not ended. False when the stored
 * value is not a whole number, has passed, lies further ahead than the embed's own time (changed
 * by hand), or storage is missing or blocked, so the button stays.
 */
function consented(e) {
  const ms = Number(e.dataset.zeynEmbedFor);
  try {
    const v = localStorage.getItem(CONSENT + e.dataset.zeynEmbedRemember);
    if (!v || !/^[0-9]{1,15}$/.test(v) || !(ms > 0)) return false;
    const left = Number(v) - Date.now();
    return left > 0 && left <= ms;
  } catch {
    return false;
  }
}

/** Puts the frame of a click to load embed in place of its button; `focus` moves focus to it. */
function loadEmbed(root, focus) {
  const template = root.querySelector(':scope > template[data-zeyn-embed]');
  if (!template) return;
  root.replaceChildren(template.content.cloneNode(true));
  if (focus) root.querySelector('iframe')?.focus();
}

/**
 * Loads the remembered embeds inside `root` whose scope holds a consent, without moving
 * focus. An embed that holds focus keeps its button, so focus is never lost.
 */
function restoreEmbeds(root) {
  root.querySelectorAll('.z-embed[data-zeyn-embed-remember]').forEach((e) => {
    if (!e.contains(document.activeElement) && consented(e)) loadEmbed(e, false);
  });
}

/**
 * Loads a click to load embed when its button is pressed and moves focus to the frame. With a
 * remembered scope the end time of the consent is stored and the other embeds of that scope load too.
 * @example
 * // <div class="z-embed" data-zeyn-embed-remember="external-video" data-zeyn-embed-for="86400000"><button data-zeyn-embed-load>..</button><template data-zeyn-embed><iframe ..></template></div>
 */
document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('[data-zeyn-embed-load]') : null;
  const root = button?.closest('.z-embed');
  if (!root) return;
  loadEmbed(root, true);
  const scope = root.dataset.zeynEmbedRemember;
  if (!scope) return;
  const ms = Number(root.dataset.zeynEmbedFor);
  if (!(ms > 0)) return;
  try {
    localStorage.setItem(CONSENT + scope, String(Date.now() + ms));
  } catch {
    return;
  }
  restoreEmbeds(document);
});
