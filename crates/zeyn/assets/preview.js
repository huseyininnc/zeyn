/**
 * Card previews (`ImageCard::preview`, `ListItem::preview`). components.js loads this module the
 * first time a fine pointer moves over such a card or one gets keyboard focus; on touch screens
 * and coarse pointers it never loads, and tapping a card just opens its link.
 *
 * After `HOVER_MS` of hover, or at once on keyboard focus, a panel opens next to the card with
 * the screen the card names, loaded from `GET /_zeyn/screen/...` only then. The server checks the
 * screen's access as for a window. Loaded previews are kept for the page's life, at most
 * `CACHE_MAX` of them. The HTML is parsed in an inert template and every frame, object, embed,
 * media element and script is removed before it is shown, so a preview never loads an iframe.
 *
 * The panel is a manual popover in the top layer, so rails and grids that clip their content
 * do not cut it; it opens on the side of the card with more room, flips at the screen edges,
 * scrolls inside itself and never scrolls the page. WCAG 1.4.13: the pointer can move onto the
 * panel without closing it, Escape closes it, and it stays until the pointer or focus leaves.
 * @module preview
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const HOVER_MS = 500;
const CLOSE_MS = 200;
const CACHE_MAX = 24;
const GAP = 8;
const fine = matchMedia('(hover: hover) and (pointer: fine)');

/** @type {Map<string, Promise<DocumentFragment | null>>} Loaded previews by address, oldest first. */
const cache = new Map();

/** @type {HTMLElement | null} */
let panel = null;
/** @type {Element | null} The card the panel belongs to, or waits for. */
let owner = null;
let openTimer = 0;
let closeTimer = 0;

/** Loads and cleans the preview of an address once; later calls get the same promise. */
function load(url) {
  const hit = cache.get(url);
  if (hit) {
    cache.delete(url);
    cache.set(url, hit);
    return hit;
  }
  const job = fetch(BASE + url, { credentials: 'same-origin', headers: { accept: 'text/html' } })
    .then((res) => (res.ok ? res.text() : null))
    .then((html) => {
      if (html == null) return null;
      const t = document.createElement('template');
      t.innerHTML = html;
      t.content.querySelectorAll('iframe, frame, object, embed, video, audio, script, template, [data-zeyn-grid]').forEach((el) => el.remove());
      t.content.querySelectorAll('[id]').forEach((el) => el.removeAttribute('id'));
      return t.content;
    })
    .catch(() => null);
  cache.set(url, job);
  while (cache.size > CACHE_MAX) cache.delete(cache.keys().next().value);
  return job;
}

/** The panel, made the first time it is needed. */
function panelEl() {
  if (panel) return panel;
  panel = document.createElement('div');
  panel.className = 'z-preview';
  panel.setAttribute('popover', 'manual');
  panel.addEventListener('pointerenter', () => clearTimeout(closeTimer));
  panel.addEventListener('pointerleave', () => closeSoon());
  document.body.append(panel);
  return panel;
}

/** Places the panel beside `card`: on the side with more room, flipped and clamped at the edges. */
function place(card) {
  const p = panelEl();
  const r = card.getBoundingClientRect();
  const vw = document.documentElement.clientWidth;
  const vh = document.documentElement.clientHeight;
  const w = Math.min(p.offsetWidth, vw - 2 * GAP);
  const h = Math.min(p.offsetHeight, vh - 2 * GAP);
  const rtl = getComputedStyle(card).direction === 'rtl';
  const after = vw - r.right - GAP;
  const before = r.left - GAP;
  let x;
  if (after >= w && (!rtl || before < w)) x = r.right + GAP;
  else if (before >= w) x = r.left - GAP - w;
  else x = after >= before ? vw - GAP - w : GAP;
  const y = Math.max(GAP, Math.min(r.top, vh - GAP - h));
  p.style.left = `${Math.round(x)}px`;
  p.style.top = `${Math.round(y)}px`;
}

/** Opens the preview of `card`, loading it the first time. */
async function open(card) {
  clearTimeout(closeTimer);
  owner = card;
  const content = await load(card.dataset.zeynPreview);
  if (owner !== card || !content || !card.isConnected || card.closest('.z-rail-dragging')) return;
  const p = panelEl();
  p.replaceChildren(content.cloneNode(true));
  if (!p.matches(':popover-open')) p.showPopover();
  place(card);
}

/** Closes the panel now. */
function close() {
  clearTimeout(openTimer);
  clearTimeout(closeTimer);
  owner = null;
  if (panel?.matches(':popover-open')) panel.hidePopover();
}

/** Closes the panel after a short delay, so the pointer can move from the card onto it. */
function closeSoon() {
  clearTimeout(openTimer);
  clearTimeout(closeTimer);
  closeTimer = setTimeout(close, CLOSE_MS);
}

/** The preview card of an event target, if any. */
function cardOf(target) {
  return target instanceof Element ? target.closest('[data-zeyn-preview]') : null;
}

/** Starts the hover timer of a card. */
function hover(card) {
  if (!fine.matches || card.closest('.z-rail-dragging')) return;
  clearTimeout(closeTimer);
  if (owner === card && panel?.matches(':popover-open')) return;
  clearTimeout(openTimer);
  owner = card;
  openTimer = setTimeout(() => open(card), HOVER_MS);
}

/** Opens a card's preview on keyboard focus. */
function focus(card) {
  if (fine.matches && card.matches(':focus-visible')) open(card);
}

document.addEventListener('pointerover', (e) => {
  const card = e.pointerType === 'mouse' || e.pointerType === 'pen' ? cardOf(e.target) : null;
  if (card && !card.contains(e.relatedTarget)) hover(card);
});

document.addEventListener('pointerout', (e) => {
  const card = cardOf(e.target);
  if (card && !card.contains(e.relatedTarget) && !panel?.contains(e.relatedTarget)) closeSoon();
});

document.addEventListener('focusin', (e) => {
  const card = cardOf(e.target);
  if (card) focus(card);
  else if (!panel?.contains(e.target)) close();
});

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && panel?.matches(':popover-open')) {
    e.preventDefault();
    close();
  }
});

document.addEventListener('pointerdown', (e) => {
  clearTimeout(openTimer);
  if (panel?.matches(':popover-open') && !panel.contains(e.target)) close();
});

addEventListener('scroll', () => owner && panel?.matches(':popover-open') && place(owner), { passive: true, capture: true });
addEventListener('resize', () => close());

document.addEventListener('zeyn:leave', () => {
  close();
  cache.clear();
});

/**
 * Called by components.js with the card and the event that loaded this module, so the first
 * hover or focus counts.
 */
export function wake(card, type) {
  if (type === 'focusin') focus(card);
  else hover(card);
}
