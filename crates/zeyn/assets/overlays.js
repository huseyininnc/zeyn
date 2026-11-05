/**
 * Tooltips and popovers. Loaded only by pages that hold one (`data-zeyn-tip`,
 * `.z-popover-button`).
 * @module overlays
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { NARROW, clickHandlers, focusables, keyHandlers } = K;

/* Tooltip ------------------------------------------------------------------------------------ */

const TIP_DELAY_MS = 300;
const LONG_PRESS_MS = 500;

/** The shown tooltip `{ trigger, tip }`, its show and hide timers, and a long press in progress. */
let tipShown = null;
let tipTimer = 0;
let tipHideTimer = 0;
let pressTimer = 0;
let pressedTip = null;
let tipWasShown = false;

/** Places a tooltip above its trigger, or below when there is no room, inside the viewport. */
function placeTip(trigger, tip) {
  const a = trigger.getBoundingClientRect();
  const w = tip.offsetWidth;
  const h = tip.offsetHeight;
  let top = a.top - h - 6;
  if (top < 4) top = a.bottom + 6;
  const left = Math.max(4, Math.min(a.left + a.width / 2 - w / 2, innerWidth - w - 4));
  tip.style.top = `${Math.round(top)}px`;
  tip.style.left = `${Math.round(left)}px`;
}

/** Shows the tooltip of a trigger (`data-zeyn-tip` names it) and hides any other. */
function showTip(trigger) {
  clearTimeout(tipTimer);
  clearTimeout(tipHideTimer);
  const tip = document.getElementById(trigger.dataset.zeynTip);
  if (!tip) return;
  if (tipShown?.tip !== tip) hideTip();
  tip.hidden = false;
  tipShown = { trigger, tip };
  placeTip(trigger, tip);
}

/** Hides the shown tooltip. */
function hideTip() {
  clearTimeout(tipTimer);
  clearTimeout(tipHideTimer);
  if (!tipShown) return;
  tipShown.tip.hidden = true;
  tipShown = null;
}

/** Hides the tooltip soon, unless the pointer moves onto it or back to its trigger (WCAG 1.4.13). */
function hideTipSoon() {
  clearTimeout(tipTimer);
  clearTimeout(tipHideTimer);
  tipHideTimer = setTimeout(hideTip, 150);
}

document.addEventListener('pointerover', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t || e.pointerType === 'touch') return;
  if (tipShown && (tipShown.tip.contains(t) || tipShown.trigger.contains(t))) return clearTimeout(tipHideTimer);
  const trigger = t.closest('[data-zeyn-tip]');
  if (!trigger) return;
  clearTimeout(tipTimer);
  tipTimer = setTimeout(() => showTip(trigger), tipShown ? 0 : TIP_DELAY_MS);
});

document.addEventListener('pointerout', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t || e.pointerType === 'touch') return;
  if (t.closest('[data-zeyn-tip]') || (tipShown && tipShown.tip.contains(t))) {
    clearTimeout(tipTimer);
    if (tipShown && document.activeElement !== tipShown.trigger) hideTipSoon();
  }
});

document.addEventListener('focusin', (e) => {
  const trigger = e.target instanceof Element ? e.target.closest('[data-zeyn-tip]') : null;
  if (trigger) showTip(trigger);
  else if (tipShown && !tipShown.tip.contains(e.target)) hideTip();
});

document.addEventListener('focusout', (e) => {
  if (tipShown && e.target === tipShown.trigger) hideTip();
});

document.addEventListener(
  'keydown',
  (e) => {
    if (e.key !== 'Escape' || !tipShown) return;
    e.preventDefault();
    e.stopPropagation();
    hideTip();
  },
  true,
);

document.addEventListener(
  'pointerdown',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    if (tipShown && t && !tipShown.tip.contains(t) && !tipShown.trigger.contains(t)) hideTip();
    const trigger = t?.closest('[data-zeyn-tip]');
    tipWasShown = !!trigger && tipShown?.trigger === trigger;
    clearTimeout(pressTimer);
    pressedTip = null;
    if (!trigger || e.pointerType !== 'touch') return;
    pressTimer = setTimeout(() => {
      pressedTip = trigger;
      showTip(trigger);
    }, LONG_PRESS_MS);
  },
  true,
);

for (const type of ['pointerup', 'pointercancel']) document.addEventListener(type, () => clearTimeout(pressTimer), true);

/** A long press shows the tooltip and does not also press the button; a tap on an info button toggles it. */
document.addEventListener(
  'click',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    const trigger = t?.closest('[data-zeyn-tip]');
    if (pressedTip && trigger === pressedTip) {
      pressedTip = null;
      e.preventDefault();
      e.stopPropagation();
      return;
    }
    if (!trigger?.classList.contains('z-tip-button')) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.detail > 0 ? tipWasShown : tipShown?.trigger === trigger) hideTip();
    else showTip(trigger);
  },
  true,
);

document.addEventListener('contextmenu', (e) => {
  if (pressedTip && e.target instanceof Element && e.target.closest('[data-zeyn-tip]') === pressedTip) e.preventDefault();
});

/* Popover ------------------------------------------------------------------------------------ */

/** The open popover `{ button, pop }`. */
let pop = null;

/** Places an open popover below its button, or above it when there is no room below. */
function placePopover() {
  if (!pop) return;
  const { button, pop: el } = pop;
  if (NARROW.matches) {
    el.style.removeProperty('top');
    el.style.removeProperty('left');
    return;
  }
  const a = button.getBoundingClientRect();
  const w = el.offsetWidth;
  const h = el.offsetHeight;
  const below = innerHeight - a.bottom - 8;
  const top = below < h && a.top - 8 > below ? Math.max(8, a.top - h - 4) : Math.min(a.bottom + 4, Math.max(8, innerHeight - h - 8));
  const rtl = getComputedStyle(button).direction === 'rtl';
  const left = Math.max(8, Math.min(rtl ? a.right - w : a.left, innerWidth - w - 8));
  el.style.top = `${Math.round(top)}px`;
  el.style.left = `${Math.round(left)}px`;
}

/** Opens a popover and moves focus to its first control, or to the dialog itself. */
function openPopover(button) {
  closePopover(false);
  const el = document.getElementById(button.getAttribute('aria-controls'));
  if (!el) return;
  el.hidden = false;
  button.setAttribute('aria-expanded', 'true');
  pop = { button, pop: el };
  placePopover();
  const first = focusables(el).find((x) => !x.matches('[data-zeyn-popover-close]'));
  (first ?? el).focus();
}

/** Closes the open popover; focus goes back to its button unless it already moved elsewhere. */
function closePopover(returnFocus = true) {
  if (!pop) return;
  const { button, pop: el } = pop;
  pop = null;
  el.hidden = true;
  button.setAttribute('aria-expanded', 'false');
  if (returnFocus) button.focus();
}

clickHandlers.push((e, t) => {
  const button = t.closest('.z-popover-button');
  if (button) {
    if (pop?.button === button) closePopover(true);
    else openPopover(button);
    return true;
  }
  if (t.closest('[data-zeyn-popover-close]')) {
    closePopover(true);
    return true;
  }
  return false;
});

keyHandlers.unshift((e, t) => {
  if (e.key !== 'Escape' || !pop || K.popup || !(pop.pop.contains(t) || pop.button === t)) return false;
  e.preventDefault();
  closePopover(true);
  return true;
});

document.addEventListener(
  'pointerdown',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    if (!pop || !t || pop.pop.contains(t) || pop.button.contains(t) || K.popup?.panel.contains(t)) return;
    closePopover(false);
  },
  true,
);

document.addEventListener('focusout', (e) => {
  if (!pop || !pop.pop.contains(e.target)) return;
  setTimeout(() => {
    const a = document.activeElement;
    if (pop && a && a !== document.body && !pop.pop.contains(a) && a !== pop.button && !K.popup?.panel.contains(a)) closePopover(false);
  });
});

window.addEventListener('resize', placePopover);
document.addEventListener('scroll', (e) => pop && !pop.pop.contains(e.target) && placePopover(), true);
document.addEventListener('zeyn:leave', () => {
  closePopover(false);
  hideTip();
});

