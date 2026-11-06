/**
 * Countdowns (`zeyn::ui::Countdown`). Loaded only by pages that hold `[data-zeyn-countdown]`.
 * One timer serves every countdown of the page: it ticks on the next whole second, writes the
 * digits, and stops while the tab is hidden (`visibilitychange`), catching up when it shows
 * again. Each countdown carries its end as an absolute moment (`data-end`), and the time left is
 * always that moment minus the browser's clock, so a page served from a cache, or one that sat in
 * a background tab, shows the right time at once. The polite live region gets new words only when the minute changes.
 * At zero the server's end state (`<template class="z-countdown-end">`) replaces the clock.
 * @module countdown
 */

/** Every countdown on the page. */
const all = new Set();
let timer = 0;

/** Days, hours, minutes and seconds in `ms`. */
function parts(ms) {
  const s = Math.max(0, Math.floor(ms / 1000));
  return [Math.floor(s / 86400), Math.floor(s / 3600) % 24, Math.floor(s / 60) % 60, s % 60];
}

/** Writes a number into a part of the clock when it changed. */
function put(el, text) {
  if (el && el.textContent !== text) el.textContent = text;
}

/** Swaps a finished countdown for its end state. */
function finish(box) {
  all.delete(box);
  const t = box.querySelector('template.z-countdown-end');
  box.classList.add('z-countdown-over');
  box.replaceChildren(t ? t.content.cloneNode(true) : '');
  box.removeAttribute('data-zeyn-countdown');
  document.dispatchEvent(new CustomEvent('zeyn:content', { detail: box }));
}

/** Writes one countdown at the server time `now`. */
function paint(box, now) {
  const left = Number(box.dataset.end) - now;
  if (left <= 0) return finish(box);
  const [d, h, m, s] = parts(left);
  const day = box.querySelector('.z-countdown-d');
  if (day) {
    put(day, String(d));
    day.hidden = d === 0;
    if (day.nextSibling?.nodeType === 3) day.nextSibling.textContent = d === 0 ? '' : `${box.dataset.day ?? 'd'} `;
  }
  put(box.querySelector('.z-countdown-h'), String(h).padStart(2, '0'));
  put(box.querySelector('.z-countdown-m'), String(m).padStart(2, '0'));
  put(box.querySelector('.z-countdown-s'), String(s).padStart(2, '0'));
  const minute = `${d}:${h}:${m}`;
  if (box.zeynMinute !== minute) {
    if (box.zeynMinute !== undefined) {
      const words = (box.dataset.textLeft ?? '').replace('{d}', String(d)).replace('{h}', String(h)).replace('{m}', String(m));
      put(box.querySelector('.z-countdown-words'), words);
    }
    box.zeynMinute = minute;
  }
}

/** Paints every countdown and plans the next tick on the next whole second. */
function tick() {
  timer = 0;
  if (document.hidden) return;
  const local = Date.now();
  for (const box of all) {
    if (box.isConnected) paint(box, local);
    else all.delete(box);
  }
  if (all.size) timer = setTimeout(tick, 1000 - (local % 1000) + 5);
}

/** Starts the countdowns under `root`. */
function setup(root) {
  const found = root.matches?.('[data-zeyn-countdown]') ? [root] : [...(root.querySelectorAll?.('[data-zeyn-countdown]') ?? [])];
  for (const box of found) all.add(box);
  if (all.size && !timer) tick();
}

setup(document);
document.addEventListener('zeyn:content', (e) => setup(e.detail ?? document));
document.addEventListener('visibilitychange', () => {
  if (!document.hidden && !timer) tick();
});
