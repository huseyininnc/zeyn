/**
 * Busy overlays: a `[data-zeyn-busy]` container shows its overlay while something inside it is
 * `aria-busy`. Loaded only by pages that hold one.
 * @module busy
 */

/* Busy overlay ------------------------------------------------------------------------------- */

const BUSY_MIN_MS = 300;

/** State of each busy container: `{ on, timer, shownAt }`. */
const busyStates = new WeakMap();

/**
 * Marks a `[data-zeyn-busy]` container busy while anything inside it is `aria-busy`, and shows
 * its overlay after the container's delay; once shown it stays at least `BUSY_MIN_MS`.
 */
function updateBusy(box) {
  const s = busyStates.get(box) ?? { on: false, timer: 0, shownAt: 0 };
  busyStates.set(box, s);
  const busy = [...box.querySelectorAll('[aria-busy="true"]')].some((el) => el.closest('[data-zeyn-busy]') === box && el !== box);
  const overlay = box.querySelector(':scope > .z-busy-overlay');
  const text = overlay?.querySelector('.z-busy-text');
  if (busy && !s.on) {
    s.on = true;
    box.setAttribute('aria-busy', 'true');
    clearTimeout(s.timer);
    s.timer = setTimeout(() => {
      if (!overlay) return;
      overlay.hidden = false;
      if (text) text.textContent = text.dataset.text ?? '';
      s.shownAt = Date.now();
    }, Number(box.dataset.zeynBusy) || 400);
  } else if (!busy && s.on) {
    s.on = false;
    box.removeAttribute('aria-busy');
    clearTimeout(s.timer);
    const hide = () => {
      if (s.on || !overlay) return;
      overlay.hidden = true;
      if (text) text.textContent = '';
    };
    if (overlay && !overlay.hidden) s.timer = setTimeout(hide, Math.max(0, BUSY_MIN_MS - (Date.now() - s.shownAt)));
  }
}

new MutationObserver((records) => {
  for (const r of records) {
    const el = r.target;
    if (!(el instanceof Element) || (r.type === 'attributes' && el.matches('[data-zeyn-busy]'))) continue;
    const box = el.closest('[data-zeyn-busy]');
    if (box) updateBusy(box);
  }
}).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['aria-busy'] });

