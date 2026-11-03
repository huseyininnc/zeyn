/**
 * Filter chips (`zeyn::ui::Chips::filter`). components.js loads this module the first time a
 * page holds one. Each chip is a link that `nav.js` opens without a page reload; the server
 * renders the pressed state from the params. This module adds what a toggle button needs on
 * top: Space presses the chip like Enter does, and after the new screen arrives focus returns
 * to the same chip (by node path, else the first chip with its value). When the chip is at the
 * same node path, the page also keeps its scroll position instead of jumping to the top.
 * @module chips
 */

/** @type {{node: string, value: string, y: number} | null} The chip pressed last. */
let pressed = null;

/** The filter chip of an event target, if any. */
function chipOf(target) {
  return target instanceof Element ? target.closest('a[data-zeyn-chip]') : null;
}

document.addEventListener('keydown', (e) => {
  const chip = e.key === ' ' && !e.repeat ? chipOf(e.target) : null;
  if (!chip) return;
  e.preventDefault();
  chip.click();
});

document.addEventListener(
  'click',
  (e) => {
    const chip = chipOf(e.target);
    if (!chip || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    pressed = { node: chip.closest('[data-zeyn-node]')?.dataset.zeynNode ?? '', value: chip.dataset.value ?? '', y: scrollY };
  },
  true,
);

document.addEventListener('zeyn:content', () => {
  if (!pressed) return;
  const { node, value, y } = pressed;
  pressed = null;
  const all = [...document.querySelectorAll('#z-main a[data-zeyn-chip]')].filter((c) => c.dataset.value === value);
  const same = all.find((c) => c.closest('[data-zeyn-node]')?.dataset.zeynNode === node);
  const chip = same ?? all[0];
  if (!chip) return;
  if (same) window.scrollTo(0, y);
  chip.focus({ preventScroll: Boolean(same) });
});
