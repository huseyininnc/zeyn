/**
 * Length controls (`Control::Length`): the value stays in pixels in a hidden native input (what
 * the form submits and what works without JavaScript); the user reads and types it in a unit
 * of the field, picked with a unit switch that the browser remembers for every length. Arrow
 * Up and Down step by 0.1, with Shift by 1, inside the field's `min` and `max` (pixels). Loaded
 * only by pages that hold one (`data-zeyn-length`).
 * @module length
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { hideNative, initializers, keyHandlers, make, parse } = K;

const UNIT_KEY = 'zeyn.length.unit';
/** Pixels in one unit, 96 to the inch. */
const PX = { mm: 96 / 25.4, cm: 96 / 2.54, in: 96, px: 1, pt: 96 / 72 };

/** The units of a field, in its order; every unit when it names none. */
function unitsOf(wrap) {
  const list = (parse(wrap.dataset.units ?? '[]') ?? []).filter((u) => Object.hasOwn(PX, u));
  return list.length ? list : Object.keys(PX);
}

/** The unit the user picked last, when the field has it, or the field's first unit. */
function preferred(wrap) {
  let saved = null;
  try {
    saved = localStorage.getItem(UNIT_KEY);
  } catch {
    /* no storage: the field's first unit */
  }
  const units = unitsOf(wrap);
  return units.includes(saved) ? saved : units[0];
}

/** A number typed with the field's decimal separator; `null` when empty or not a finite number. */
function readNumber(wrap, text) {
  const plain = text.replace(/\s/g, '').split(wrap.dataset.decimal ?? '.').join('.');
  if (!/^[-+]?(?:\d+\.?\d*|\.\d+)$/.test(plain)) return null;
  const n = Number(plain);
  return Number.isFinite(n) ? n : null;
}

/** A number with at most `scale` decimals and the field's decimal separator. */
function writeNumber(wrap, n, scale) {
  return String(Number(n.toFixed(scale))).replace('.', wrap.dataset.decimal ?? '.');
}

/** Keeps pixels inside the field's `min` and `max`. */
function clamp(wrap, px) {
  const d = wrap.dataset;
  if (d.min !== undefined) px = Math.max(Number(d.min), px);
  if (d.max !== undefined) px = Math.min(Number(d.max), px);
  return px;
}

/** Shows the pixels of the native input in the chosen unit. */
function show(wrap) {
  const native = wrap.querySelector('.z-length-px');
  const shown = wrap.querySelector('.z-length-value');
  const unit = wrap.querySelector('.z-length-unit').value;
  const px = readNumber(wrap, native.value);
  if (px === null) {
    if (native.value === '') shown.value = '';
    shown.removeAttribute('aria-valuenow');
    shown.removeAttribute('aria-valuetext');
    return;
  }
  const n = Number((px / PX[unit]).toFixed(2));
  shown.value = writeNumber(wrap, n, 2);
  shown.setAttribute('aria-valuenow', String(n));
  shown.setAttribute('aria-valuetext', `${shown.value} ${unit}`);
}

/** Turns `.z-length` fields into a number in the chosen unit and a unit switch. */
function initLengths(root) {
  root.querySelectorAll('.z-length:not([data-ready])').forEach((wrap) => {
    const native = wrap.querySelector('.z-length-px');
    if (!native) return;
    wrap.dataset.ready = '';
    const shown = make('input', 'z-num z-length-value', {
      type: 'text',
      role: 'spinbutton',
      id: native.id,
      inputmode: 'decimal',
      autocomplete: 'off',
      dir: 'ltr',
      'data-testid': `${native.dataset.testid ?? ''}.input`,
    });
    for (const a of ['aria-describedby', 'aria-required', 'aria-invalid']) if (native.hasAttribute(a)) shown.setAttribute(a, native.getAttribute(a));
    shown.readOnly = native.readOnly;
    const unit = make('select', 'z-length-unit', { id: `${native.id}-unit`, 'aria-label': wrap.dataset.textUnit ?? '', 'data-testid': `${native.dataset.testid ?? ''}.unit` });
    for (const u of unitsOf(wrap)) unit.append(make('option', '', { value: u, text: u }));
    unit.value = preferred(wrap);
    unit.disabled = native.readOnly;
    native.removeAttribute('id');
    hideNative(native);
    wrap.querySelector('.z-affix')?.remove();
    wrap.prepend(shown);
    wrap.append(unit);
    wrap.zeynSync = () => show(wrap);
    show(wrap);
  });
}

/** Writes what the user typed into the native input as pixels, or as typed when unreadable. */
function typed(shown) {
  const wrap = shown.closest('.z-length');
  const native = wrap.querySelector('.z-length-px');
  const unit = wrap.querySelector('.z-length-unit').value;
  const n = readNumber(wrap, shown.value);
  native.value = n === null ? shown.value : writeNumber(wrap, n * PX[unit], Number(wrap.dataset.scale ?? 4));
  native.dispatchEvent(new Event('input', { bubbles: true }));
}

/** Arrow Up and Down step by 0.1 of the unit, with Shift by 1 (ARIA spinbutton). */
function lengthKey(e, shown) {
  const sign = { ArrowUp: 1, ArrowDown: -1 }[e.key];
  if (sign === undefined || e.altKey || shown.readOnly) return false;
  e.preventDefault();
  const wrap = shown.closest('.z-length');
  const native = wrap.querySelector('.z-length-px');
  const unit = wrap.querySelector('.z-length-unit').value;
  const now = readNumber(wrap, shown.value) ?? 0;
  const next = Math.round((now + sign * (e.shiftKey ? 1 : 0.1)) * 100) / 100;
  native.value = writeNumber(wrap, clamp(wrap, next * PX[unit]), Number(wrap.dataset.scale ?? 4));
  show(wrap);
  native.dispatchEvent(new Event('input', { bubbles: true }));
  native.dispatchEvent(new Event('change', { bubbles: true }));
  return true;
}

initializers.push(initLengths);

keyHandlers.push((e, t) => t.matches('.z-length-value') && lengthKey(e, t));

document.addEventListener('input', (e) => {
  const t = e.target;
  if (t instanceof HTMLInputElement && t.matches('.z-length-value')) typed(t);
});

document.addEventListener('change', (e) => {
  const t = e.target;
  if (t instanceof HTMLInputElement && t.matches('.z-length-value')) {
    e.stopPropagation();
    const native = t.closest('.z-length').querySelector('.z-length-px');
    native.dispatchEvent(new Event('change', { bubbles: true }));
    return;
  }
  if (!(t instanceof HTMLSelectElement) || !t.matches('.z-length-unit')) return;
  e.stopPropagation();
  try {
    localStorage.setItem(UNIT_KEY, t.value);
  } catch {
    /* no storage: the unit is not kept */
  }
  document.querySelectorAll('.z-length[data-ready]').forEach((wrap) => {
    const select = wrap.querySelector('.z-length-unit');
    if (select !== t && [...select.options].some((o) => o.value === t.value)) select.value = t.value;
    show(wrap);
  });
}, true);

initLengths(document);
