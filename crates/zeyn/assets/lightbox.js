/**
 * Lightbox (`Image::lightbox`, the media of `Post`). Loaded only by pages that hold
 * `[data-zeyn-lightbox]`.
 *
 * A trigger is a link to the full file around the thumbnail, so without script it opens the
 * file. With script, a click opens one modal `<dialog>` for the page (W3C APG dialog pattern):
 * the items are the triggers of the trigger's gallery (`[data-zeyn-gallery]`, a post), else of
 * the container node around the trigger's own node (a `Row`, `Stack` or `AutoGrid`). Only the open item is in the dialog; its neighbours are fetched ahead as images (videos
 * never are). The thumbnail shows at once and the full file covers it when it has loaded. Tab
 * stays in the dialog, Escape and the close button close it and focus returns to the trigger
 * (or to the post that held it, when the feed unloaded the trigger meanwhile). Arrow keys and
 * swipes move between items; double click, double tap, the zoom button and `+`, `-`, `0` zoom,
 * and a zoomed image pans by dragging or with the arrow keys. Motion follows
 * `prefers-reduced-motion` through the stylesheet.
 * @module lightbox
 */

const ZOOM = 2.5;
const SWIPE_PX = 50;
const PAN_PX = 40;

/** @type {{dialog: HTMLDialogElement, stage: HTMLElement, caption: HTMLElement, count: HTMLElement, zoom: HTMLButtonElement, prev: HTMLButtonElement, next: HTMLButtonElement} | null} */
let ui = null;
/** The open state: the triggers, the index shown, the opener, the zoom and pan. */
const open = { items: [], at: 0, opener: null, post: null, scale: 1, x: 0, y: 0, texts: {} };
/** Images fetched ahead, by address, at most a few. */
const ahead = new Map();

/** Parses the JSON texts of the nearest holder, or an empty object. */
function textsOf(el) {
  try {
    return JSON.parse(el.closest('[data-zeyn-lightbox-texts]')?.dataset.zeynLightboxTexts ?? '{}') ?? {};
  } catch {
    return {};
  }
}

/** Replaces `{name}` placeholders. */
function fill(template, values) {
  return (template ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in values ? String(values[k]) : m));
}

/** A button with a label and a symbol hidden from screen readers. */
function button(cls, symbol) {
  const b = document.createElement('button');
  b.type = 'button';
  b.className = `z-lightbox-button ${cls}`;
  const s = document.createElement('span');
  s.setAttribute('aria-hidden', 'true');
  s.textContent = symbol;
  b.append(s);
  return b;
}

/** Builds the dialog once, the first time a lightbox opens. */
function build() {
  const dialog = document.createElement('dialog');
  dialog.className = 'z-lightbox';
  const bar = document.createElement('div');
  bar.className = 'z-lightbox-bar';
  const count = document.createElement('p');
  count.className = 'z-lightbox-count';
  count.setAttribute('aria-live', 'polite');
  const zoom = button('z-lightbox-zoom', '+');
  zoom.setAttribute('aria-pressed', 'false');
  const close = button('z-lightbox-close', '×');
  bar.append(count, zoom, close);
  const stage = document.createElement('div');
  stage.className = 'z-lightbox-stage';
  const prev = button('z-lightbox-prev', '‹');
  const next = button('z-lightbox-next', '›');
  const caption = document.createElement('p');
  caption.className = 'z-lightbox-caption';
  dialog.append(bar, stage, prev, next, caption);
  document.body.append(dialog);
  ui = { dialog, stage, caption, count, zoom, prev, next };
  close.addEventListener('click', () => dialog.close());
  prev.addEventListener('click', () => move(-1));
  next.addEventListener('click', () => move(1));
  zoom.addEventListener('click', () => setZoom(open.scale > 1 ? 1 : ZOOM));
  dialog.addEventListener('close', closed);
  dialog.addEventListener('keydown', keys);
  dialog.addEventListener('click', (e) => {
    if (e.target === dialog || e.target === stage) dialog.close();
  });
  dialog.addEventListener('dblclick', (e) => {
    if (e.target.closest?.('.z-lightbox-full')) setZoom(open.scale > 1 ? 1 : ZOOM, e);
  });
  pointers(stage);
}

/** What an item shows: its full file, thumbnail, alternative text, caption and video. */
function itemOf(trigger) {
  const img = trigger.querySelector('img');
  return {
    full: trigger.getAttribute('href') ?? '',
    low: img?.currentSrc || img?.getAttribute('src') || '',
    alt: img?.getAttribute('alt') ?? '',
    caption: trigger.dataset.caption ?? '',
    video: trigger.dataset.video ?? '',
    w: Number(img?.getAttribute('width')) || 0,
    h: Number(img?.getAttribute('height')) || 0,
  };
}

/** Fetches the full images of the neighbours of the open item, keeping at most four. */
function prefetch() {
  for (const step of [1, -1]) {
    const t = open.items[open.at + step];
    if (!t) continue;
    const item = itemOf(t);
    if (item.video || ahead.has(item.full)) continue;
    const img = new Image();
    img.decoding = 'async';
    img.src = item.full;
    ahead.set(item.full, img);
    while (ahead.size > 4) ahead.delete(ahead.keys().next().value);
  }
}

/** Shows item `open.at`: its thumbnail first, the full file over it once loaded. */
function show() {
  const { stage, caption, count, prev, next, zoom } = ui;
  const item = itemOf(open.items[open.at]);
  stage.querySelector('video')?.pause();
  const figure = document.createElement('figure');
  figure.className = 'z-lightbox-item';
  if (item.w && item.h) figure.style.aspectRatio = `${item.w} / ${item.h}`;
  if (item.video) {
    const video = document.createElement('video');
    video.className = 'z-lightbox-video';
    video.controls = true;
    video.playsInline = true;
    video.preload = 'metadata';
    if (item.low) video.poster = item.low;
    video.src = item.video;
    if (item.alt) video.setAttribute('aria-label', item.alt);
    figure.append(video);
  } else {
    if (item.low && item.low !== item.full) {
      const low = document.createElement('img');
      low.className = 'z-lightbox-low';
      low.alt = '';
      low.src = item.low;
      figure.append(low);
    }
    const full = document.createElement('img');
    full.className = 'z-lightbox-full';
    full.alt = item.alt;
    full.decoding = 'async';
    full.draggable = false;
    full.addEventListener('load', () => full.classList.add('z-lightbox-loaded'), { once: true });
    full.src = item.full;
    if (full.complete) full.classList.add('z-lightbox-loaded');
    figure.append(full);
  }
  stage.replaceChildren(figure);
  caption.textContent = item.caption;
  caption.hidden = !item.caption;
  const many = open.items.length > 1;
  count.textContent = many ? fill(open.texts.position, { n: open.at + 1, count: open.items.length }) : '';
  prev.hidden = next.hidden = !many;
  prev.disabled = open.at === 0;
  next.disabled = open.at === open.items.length - 1;
  zoom.hidden = Boolean(item.video);
  setZoom(1);
  prefetch();
}

/** Moves to the item `step` away, within the ends. */
function move(step) {
  const to = open.at + step;
  if (to < 0 || to >= open.items.length || to === open.at) return;
  open.at = to;
  show();
}

/** Zooms the image to `scale` (1 resets), around the pointer of `e` when given. */
function setZoom(scale, e) {
  const img = ui.stage.querySelector('.z-lightbox-full');
  open.scale = img ? scale : 1;
  open.x = open.y = 0;
  ui.zoom.setAttribute('aria-pressed', String(open.scale > 1));
  ui.zoom.setAttribute('aria-label', open.scale > 1 ? open.texts.zoom_out ?? '' : open.texts.zoom_in ?? '');
  ui.zoom.firstChild.textContent = open.scale > 1 ? '−' : '+';
  ui.dialog.classList.toggle('z-lightbox-zoomed', open.scale > 1);
  if (!img) return;
  if (e && open.scale > 1) {
    const box = img.getBoundingClientRect();
    img.style.transformOrigin = `${e.clientX - box.left}px ${e.clientY - box.top}px`;
  } else {
    img.style.transformOrigin = '';
  }
  pan(0, 0);
}

/** Moves a zoomed image by `dx`, `dy` pixels, kept within its zoomed size. */
function pan(dx, dy) {
  const img = ui.stage.querySelector('.z-lightbox-full');
  if (!img) return;
  const limitX = (img.clientWidth * (open.scale - 1)) / 2;
  const limitY = (img.clientHeight * (open.scale - 1)) / 2;
  open.x = Math.max(-limitX, Math.min(limitX, open.x + dx));
  open.y = Math.max(-limitY, Math.min(limitY, open.y + dy));
  img.style.transform = open.scale > 1 ? `translate(${open.x}px, ${open.y}px) scale(${open.scale})` : '';
}

/** Keys of the open dialog: arrows move (or pan when zoomed), zoom keys, Tab stays inside. */
function keys(e) {
  if (e.altKey || e.ctrlKey || e.metaKey) return;
  const rtl = getComputedStyle(ui.dialog).direction === 'rtl' ? -1 : 1;
  const zoomed = open.scale > 1;
  const onVideo = e.target.closest?.('video');
  let handled = true;
  if (e.key === 'ArrowRight' && !onVideo) zoomed ? pan(-PAN_PX, 0) : move(rtl);
  else if (e.key === 'ArrowLeft' && !onVideo) zoomed ? pan(PAN_PX, 0) : move(-rtl);
  else if (e.key === 'ArrowUp' && zoomed) pan(0, PAN_PX);
  else if (e.key === 'ArrowDown' && zoomed) pan(0, -PAN_PX);
  else if (e.key === 'Home' && !onVideo) move(-open.at);
  else if (e.key === 'End' && !onVideo) move(open.items.length - 1 - open.at);
  else if (e.key === '+' || e.key === '=') setZoom(ZOOM);
  else if (e.key === '-' || e.key === '0') setZoom(1);
  else if (e.key === 'Tab') trap(e);
  else handled = false;
  if (handled && e.key !== 'Tab') e.preventDefault();
}

/** Keeps Tab and Shift+Tab inside the dialog, wrapping at the ends. */
function trap(e) {
  const stops = [...ui.dialog.querySelectorAll('button, video[controls]')].filter((el) => !el.hidden && !el.disabled);
  if (!stops.length) return;
  const first = stops[0];
  const last = stops[stops.length - 1];
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

/**
 * Swipes and drags on the stage: a horizontal swipe moves between items, a drag pans a zoomed
 * image, two taps within 300 ms zoom (touch screens send no `dblclick` before that delay).
 */
function pointers(stage) {
  let start = null;
  let lastTap = 0;
  stage.addEventListener('pointerdown', (e) => {
    if (e.target.closest('video')) return;
    start = { id: e.pointerId, x: e.clientX, y: e.clientY, px: e.clientX, py: e.clientY };
    stage.setPointerCapture(e.pointerId);
  });
  stage.addEventListener('pointermove', (e) => {
    if (!start || e.pointerId !== start.id || open.scale <= 1) return;
    pan(e.clientX - start.px, e.clientY - start.py);
    start.px = e.clientX;
    start.py = e.clientY;
  });
  const end = (e) => {
    if (!start || e.pointerId !== start.id) return;
    const dx = e.clientX - start.x;
    const dy = e.clientY - start.y;
    start = null;
    if (e.type === 'pointercancel') return;
    if (open.scale <= 1 && Math.abs(dx) > SWIPE_PX && Math.abs(dx) > Math.abs(dy)) {
      const rtl = getComputedStyle(ui.dialog).direction === 'rtl' ? -1 : 1;
      move(dx < 0 ? rtl : -rtl);
      return;
    }
    if (e.pointerType !== 'mouse' && Math.abs(dx) < 10 && Math.abs(dy) < 10) {
      const now = e.timeStamp;
      if (now - lastTap < 300 && e.target.closest('.z-lightbox-full')) setZoom(open.scale > 1 ? 1 : ZOOM, e);
      lastTap = now;
    }
  };
  stage.addEventListener('pointerup', end);
  stage.addEventListener('pointercancel', end);
}

/** Opens the lightbox on `trigger` with the items of its gallery. */
function openAt(trigger) {
  if (!ui) build();
  const gallery = trigger.closest('[data-zeyn-gallery]') ?? trigger.closest('[data-zeyn-node]')?.parentElement?.closest('[data-zeyn-node]');
  open.items = gallery ? [...gallery.querySelectorAll('[data-zeyn-lightbox]')] : [trigger];
  open.at = Math.max(0, open.items.indexOf(trigger));
  open.opener = trigger;
  open.post = trigger.closest('[data-zeyn-key]')?.dataset.zeynKey ?? null;
  open.texts = textsOf(trigger);
  const { dialog } = ui;
  dialog.setAttribute('aria-label', open.texts.label ?? '');
  dialog.querySelector('.z-lightbox-close').setAttribute('aria-label', open.texts.close ?? '');
  ui.prev.setAttribute('aria-label', open.texts.previous ?? '');
  ui.next.setAttribute('aria-label', open.texts.next ?? '');
  dialog.dir = getComputedStyle(trigger).direction;
  show();
  dialog.showModal();
  dialog.querySelector('.z-lightbox-close').focus();
}

/** After the dialog closed: stops a video and returns focus to what opened it. */
function closed() {
  ui.stage.querySelector('video')?.pause();
  ui.stage.replaceChildren();
  const opener = open.opener;
  open.items = [];
  if (opener?.isConnected) {
    opener.focus({ preventScroll: true });
    return;
  }
  const key = open.post;
  const post = key ? document.querySelector(`[data-zeyn-key="${CSS.escape(key)}"]`) : null;
  post?.focus({ preventScroll: true });
}

document.addEventListener(
  'click',
  (e) => {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    const trigger = e.target.closest?.('a[data-zeyn-lightbox]');
    if (!trigger || typeof HTMLDialogElement !== 'function') return;
    e.preventDefault();
    e.stopPropagation();
    openAt(trigger);
  },
  true,
);
