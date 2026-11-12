/**
 * The drag core: small helpers that components which move or drag things build on (grid column
 * moves, rail drag scrolling, and later sortable lists, boards, tree moves, dock tabs and free
 * surfaces). Loaded only by the modules that import it, so pages without them never fetch it.
 *
 * - `press` watches a pointer press and starts a drag: a mouse or pen after it moves `threshold`
 *   pixels, a touch after a `longPress` hold that stays within `tolerance` pixels, so a swipe
 *   still scrolls. Scrolling before the drag starts cancels it.
 * - `capture` follows a started drag with pointer capture: moves are coalesced to one call per
 *   animation frame, release, cancel, a lost capture and Escape end it.
 * - `swallowClick` cancels the click that follows the release of a drag.
 * - `announce` speaks a text in one polite live region per page, at most every `ANNOUNCE_MS`;
 *   callers pass texts from the locale files.
 *
 * Keyboard users never depend on dragging: every component that drags also offers a move menu
 * and keys (WCAG 2.5.7). `.z-drag-handle` turns off touch panning on a handle (`zeyn.css`), and
 * nothing here writes HTML.
 * @module drag
 */

/** Milliseconds between two announcements; a newer text waits and replaces an older one. */
const ANNOUNCE_MS = 500;

/**
 * Watches the press of `e` (a `pointerdown`) and calls `onStart(event)` once it becomes a drag.
 * Returns a function that stops watching.
 * @param {PointerEvent} e
 * @param {{threshold?: number, longPress?: number, tolerance?: number, axis?: 'x' | 'y', onStart: (ev: PointerEvent) => void}} options
 * @returns {() => void}
 * @example
 * el.addEventListener('pointerdown', (e) => press(e, { onStart: (ev) => capture(el, ev, move, end) }));
 */
export function press(e, { threshold = 6, longPress = 250, tolerance = 5, axis, onStart }) {
  const id = e.pointerId;
  const x0 = e.clientX;
  const y0 = e.clientY;
  const touch = e.pointerType === 'touch';
  let last = e;
  let timer = 0;
  const watch = (on) => {
    const m = on ? 'addEventListener' : 'removeEventListener';
    document[m]('pointermove', move, true);
    document[m]('pointerup', stop, true);
    document[m]('pointercancel', stop, true);
    document[m]('scroll', stop, true);
  };
  const stop = (ev) => {
    if (ev?.pointerId != null && ev.pointerId !== id) return;
    clearTimeout(timer);
    watch(false);
  };
  const begin = (ev) => {
    stop();
    onStart(ev);
  };
  const move = (ev) => {
    if (ev.pointerId !== id) return;
    last = ev;
    const dx = Math.abs(ev.clientX - x0);
    const dy = Math.abs(ev.clientY - y0);
    if (touch) {
      if (dx > tolerance || dy > tolerance) stop();
      return;
    }
    if ((axis !== 'y' && dx >= threshold) || (axis !== 'x' && dy >= threshold)) begin(ev);
  };
  watch(true);
  if (touch) timer = setTimeout(() => begin(last), longPress);
  return stop;
}

/**
 * Follows a started drag of the pointer of `e` on `el`: `move(event)` at most once per animation
 * frame with the latest event, then `end(event, cancelled)` once, where `cancelled` is true for
 * `pointercancel`, a lost capture or Escape. A move still waiting for its frame runs before a
 * release ends the drag. After Escape the click of the release is swallowed. Returns a function
 * that ends the drag as cancelled.
 * @param {Element} el
 * @param {PointerEvent} e
 * @param {(ev: PointerEvent) => void} move
 * @param {(ev: Event, cancelled: boolean) => void} end
 * @returns {() => void}
 */
export function capture(el, e, move, end) {
  const id = e.pointerId;
  let frame = 0;
  let last = e;
  let done = false;
  try {
    el.setPointerCapture(id);
  } catch {}
  const flush = () => {
    frame = 0;
    move(last);
  };
  const onMove = (ev) => {
    if (ev.pointerId !== id) return;
    last = ev;
    frame ||= requestAnimationFrame(flush);
  };
  const finish = (ev, cancelled) => {
    if (done) return;
    done = true;
    el.removeEventListener('pointermove', onMove);
    el.removeEventListener('pointerup', onUp);
    el.removeEventListener('pointercancel', onUp);
    el.removeEventListener('lostpointercapture', onUp);
    document.removeEventListener('keydown', onKey, true);
    if (frame) {
      cancelAnimationFrame(frame);
      if (cancelled) frame = 0;
      else flush();
    }
    end(ev, cancelled);
  };
  const onUp = (ev) => {
    if (ev.pointerId === id) finish(ev, ev.type !== 'pointerup');
  };
  const onKey = (ev) => {
    if (ev.key !== 'Escape') return;
    ev.preventDefault();
    ev.stopPropagation();
    finish(ev, true);
    afterRelease(id, swallowClick);
  };
  el.addEventListener('pointermove', onMove);
  el.addEventListener('pointerup', onUp);
  el.addEventListener('pointercancel', onUp);
  el.addEventListener('lostpointercapture', onUp);
  document.addEventListener('keydown', onKey, true);
  return () => finish(e, true);
}

/** Runs `run` when the pointer `id` is released or cancelled. */
function afterRelease(id, run) {
  const once = (ev) => {
    if (ev.pointerId !== id) return;
    document.removeEventListener('pointerup', once, true);
    document.removeEventListener('pointercancel', once, true);
    if (ev.type === 'pointerup') run();
  };
  document.addEventListener('pointerup', once, true);
  document.addEventListener('pointercancel', once, true);
}

/**
 * Cancels the click the browser sends right after the release of a drag, so the link or button
 * under the pointer does not run. Call it from the `pointerup` of the drag.
 */
export function swallowClick() {
  const swallow = (c) => {
    c.preventDefault();
    c.stopPropagation();
  };
  document.addEventListener('click', swallow, { capture: true, once: true });
  setTimeout(() => document.removeEventListener('click', swallow, true), 0);
}

let region = null;
let spoken = 0;
let waiting = 0;
let next = '';

/**
 * Speaks `text` in the page's polite live region for drags, created on the first call. Calls
 * closer together than `ANNOUNCE_MS` keep only the newest text, spoken when the time is up.
 * @param {string} text A text from the locale files, filled in by the caller.
 */
export function announce(text) {
  next = text;
  if (waiting) return;
  const wait = spoken + ANNOUNCE_MS - performance.now();
  if (wait > 0) {
    waiting = setTimeout(speak, wait);
    return;
  }
  speak();
}

/** Puts the waiting text into the live region. */
function speak() {
  waiting = 0;
  spoken = performance.now();
  if (!region?.isConnected) {
    region = document.createElement('div');
    region.className = 'z-sr-only';
    region.setAttribute('role', 'status');
    region.setAttribute('aria-live', 'polite');
    document.body.append(region);
  }
  region.textContent = next;
}
