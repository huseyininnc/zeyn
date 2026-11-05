/**
 * Number fields, growing text areas, masked text and passwords. Loaded only by pages that hold
 * one (`data-zeyn-number`, `data-zeyn-autosize`, `.z-mask`, `data-zeyn-password`).
 * @module inputs
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { clickHandlers, fill, hideNative, initializers, keyHandlers, lang, make, parse } = K;

/* Numbers ------------------------------------------------------------------------------------ */

/** Reads a number typed in the field's locale; `null` when empty or unreadable. */
function readNumber(wrap, text) {
  const d = wrap.dataset;
  const plain = text.split(d.group).join('').replace(/\s/g, '').replace(d.decimal, '.');
  if (!plain || plain === '-' || !/^-?\d*\.?\d*$/.test(plain)) return null;
  return Number(plain);
}

/** Writes a number in the field's locale: groups and the field's decimals. */
function writeNumber(wrap, n) {
  const d = wrap.dataset;
  const scale = Number(d.scale);
  const [whole, frac] = Math.abs(n).toFixed(scale).split('.');
  const grouped = d.grouped === 'true' ? whole.replace(/\B(?=(\d{3})+(?!\d))/g, d.group) : whole;
  return `${n < 0 ? '-' : ''}${grouped}${frac ? d.decimal + frac : ''}`;
}

/** Formats while typing: adds group separators and keeps at most the field's decimals. */
function formatNumber(input) {
  const wrap = input.closest('[data-zeyn-number]');
  const d = wrap.dataset;
  const scale = Number(d.scale);
  const v = input.value;
  const caret = input.selectionStart ?? v.length;
  const keep = (s) => s.replace(new RegExp(`[^0-9${scale ? d.decimal.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') : ''}-]`, 'g'), '');
  const before = keep(v.slice(0, caret)).length;
  let clean = keep(v);
  const negative = clean.startsWith('-');
  clean = clean.replace(/-/g, '');
  let [whole, ...rest] = clean.split(d.decimal);
  let frac = rest.join('');
  if (scale) frac = frac.slice(0, scale);
  whole = whole.replace(/^0+(?=\d)/, '');
  const grouped = d.grouped === 'true' ? whole.replace(/\B(?=(\d{3})+(?!\d))/g, d.group) : whole;
  const text = `${negative ? '-' : ''}${grouped}${clean.includes(d.decimal) && scale ? d.decimal + frac : ''}`;
  if (text === v) return;
  input.value = text;
  let seen = 0;
  let pos = 0;
  while (pos < text.length && seen < before) {
    if (text[pos] !== d.group) seen++;
    pos++;
  }
  input.setSelectionRange(pos, pos);
  const n = readNumber(wrap, text);
  if (n !== null) input.setAttribute('aria-valuenow', String(n));
  else input.removeAttribute('aria-valuenow');
}

/** Steps a number field by whole units, inside its `min` and `max`. */
function stepNumber(input, by) {
  if (input.readOnly || input.disabled) return;
  const wrap = input.closest('[data-zeyn-number]');
  const d = wrap.dataset;
  let n = (readNumber(wrap, input.value) ?? 0) + by;
  if (d.min !== undefined) n = Math.max(Number(d.min), n);
  if (d.max !== undefined) n = Math.min(Number(d.max), n);
  input.value = writeNumber(wrap, n);
  input.setAttribute('aria-valuenow', String(n));
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
}

/** Arrow Up and Down step by one, Page Up and Down by ten (ARIA spinbutton). */
function numberKey(e, input) {
  const by = { ArrowUp: 1, ArrowDown: -1, PageUp: 10, PageDown: -10 }[e.key];
  if (by === undefined || e.altKey) return false;
  e.preventDefault();
  stepNumber(input, by);
  return true;
}

/* Text areas --------------------------------------------------------------------------------- */

/** Grows a text area with its text (where CSS `field-sizing` is not supported) and updates its counter. */
function textareaInput(area) {
  if (!CSS.supports('field-sizing', 'content')) {
    area.style.height = 'auto';
    area.style.height = `${area.scrollHeight + 2}px`;
  }
  const counter = area.id && document.getElementById(`${area.id}-count`);
  if (!counter) return;
  const max = Number(counter.dataset.zeynCounter);
  const n = [...area.value].length;
  counter.textContent = fill(counter.dataset.text, { n, max });
  const near = n >= max * 0.9;
  counter.classList.toggle('z-near', near);
  if (near) counter.setAttribute('aria-live', 'polite');
  else counter.removeAttribute('aria-live');
}

/** Sets up text areas that grow with their text. */
function initTextareas(root) {
  if (CSS.supports('field-sizing', 'content')) return;
  root.querySelectorAll('[data-zeyn-autosize]').forEach(textareaInput);
}

/* Masked text -------------------------------------------------------------------------------- */

/** Slots and literals of a mask pattern: `9` digit, `a` letter, `A` upper case letter, `*` either. */
function maskTokens(pattern) {
  const out = [];
  for (let i = 0; i < pattern.length; i++) {
    const c = pattern[i];
    if (c === '\\' && i + 1 < pattern.length) out.push({ lit: pattern[++i] });
    else if ('9aA*'.includes(c)) out.push({ slot: c });
    else out.push({ lit: c });
  }
  return out;
}

/** Whether a slot takes a character. */
function slotTakes(slot, c) {
  if (slot === '9') return /^\d$/.test(c);
  if (slot === '*') return /^[\p{L}\p{N}]$/u.test(c);
  return /^\p{L}$/u.test(c);
}

/** The raw characters of typed text, as the server reads them (`ui::mask_raw`). */
function maskRaw(tokens, text) {
  let raw = '';
  let at = 0;
  outer: for (const c of text) {
    while (tokens[at]?.lit !== undefined) {
      if (tokens[at++].lit === c) continue outer;
    }
    const t = tokens[at];
    if (t && slotTakes(t.slot, c)) {
      raw += t.slot === 'A' ? c.toLocaleUpperCase(lang()) : c;
      at++;
    } else if (/[\p{L}\p{N}]/u.test(c)) break;
  }
  return raw;
}

/** Raw characters shown through the pattern, with the literals up to the last one typed. */
function maskFormat(tokens, raw) {
  let out = '';
  let i = 0;
  for (const t of tokens) {
    if (i >= raw.length) break;
    if (t.lit !== undefined) out += t.lit;
    else out += raw[i++];
  }
  return out;
}

/** Turns `[data-zeyn-mask]` inputs into masked inputs over the raw value the form submits. */
function initMasks(root) {
  root.querySelectorAll('.z-mask:not([data-ready])').forEach((wrap) => {
    const native = wrap.querySelector('.z-mask-value');
    if (!native) return;
    wrap.dataset.ready = '';
    const tokens = maskTokens(wrap.dataset.zeynMask);
    const shown = make('input', 'z-mask-input', {
      type: 'text',
      id: native.id,
      autocomplete: 'off',
      spellcheck: 'false',
      inputmode: native.inputMode || 'text',
      dir: 'ltr',
      placeholder: wrap.dataset.example ?? '',
      'data-testid': `${native.dataset.testid ?? ''}.input`,
    });
    for (const a of ['aria-describedby', 'aria-required', 'aria-invalid']) if (native.hasAttribute(a)) shown.setAttribute(a, native.getAttribute(a));
    shown.readOnly = native.readOnly;
    shown.value = maskFormat(tokens, maskRaw(tokens, native.value));
    native.removeAttribute('id');
    hideNative(native);
    wrap.prepend(shown);
    wrap.zeynTokens = tokens;
  });
}

/**
 * Reformats a masked input as the user types, keeps the caret after the same raw character and
 * writes the raw value. Backspace over a literal removes the character before it.
 */
function maskInput(shown, e) {
  const wrap = shown.closest('.z-mask');
  const tokens = wrap.zeynTokens;
  const native = wrap.querySelector('.z-mask-value');
  const caret = shown.selectionStart ?? shown.value.length;
  let before = maskRaw(tokens, shown.value.slice(0, caret)).length;
  let raw = maskRaw(tokens, shown.value);
  if (e.inputType === 'deleteContentBackward' && raw === native.value && before > 0) {
    raw = raw.slice(0, before - 1) + raw.slice(before);
    before--;
  }
  const text = maskFormat(tokens, raw);
  if (text !== shown.value) {
    shown.value = text;
    let seen = 0;
    let pos = 0;
    for (const t of tokens) {
      if (seen >= before || pos >= text.length) break;
      pos++;
      if (t.lit === undefined) seen++;
    }
    shown.setSelectionRange(pos, pos);
  }
  if (native.value === raw) return;
  native.value = raw;
  native.dispatchEvent(new Event('input', { bubbles: true }));
  native.dispatchEvent(new Event('change', { bubbles: true }));
}

/* Password ----------------------------------------------------------------------------------- */

/** Shows the show and hide buttons of password inputs, which need this script. */
function initPasswords(root) {
  root.querySelectorAll('[data-zeyn-password]:not([data-ready])').forEach((wrap) => {
    wrap.dataset.ready = '';
    const button = wrap.querySelector('.z-password-toggle');
    if (button) button.hidden = false;
  });
}

/** Shows or hides the password (APG toggle button: the name stays, `aria-pressed` changes). */
function togglePassword(button) {
  const input = button.closest('[data-zeyn-password]').querySelector('input');
  const show = button.getAttribute('aria-pressed') !== 'true';
  input.type = show ? 'text' : 'password';
  button.setAttribute('aria-pressed', String(show));
}

/**
 * A rough strength from 1 (weak) to 4 (strong): length, kinds of characters and obvious
 * patterns. Only a hint; the server's rules decide what is accepted.
 */
function strengthOf(pw) {
  const len = [...pw].length;
  const kinds = [/\p{Ll}/u, /\p{Lu}/u, /\p{N}/u, /[^\p{L}\p{N}]/u].filter((r) => r.test(pw)).length;
  if (len < 8 || /^(.)\1*$/u.test(pw) || /^(?:password|parola|qwerty|123456|111111|abc123)/i.test(pw)) return 1;
  return Math.min(4, 1 + (len >= 12) + (kinds >= 3) + (len >= 16 || (len >= 12 && kinds === 4)));
}

/** Updates the strength meter under a new password input. */
function meterInput(input) {
  const meter = document.querySelector(`[data-zeyn-strength="${CSS.escape(input.id)}"]`);
  if (!meter) return;
  const level = input.value ? strengthOf(input.value) : 0;
  meter.hidden = level === 0;
  if (meter.dataset.level === String(level)) return;
  meter.dataset.level = String(level);
  const levels = parse(meter.dataset.levels ?? '[]') ?? [];
  meter.querySelector('.z-strength-text').textContent = level ? (levels[level - 1] ?? '') : '';
}

/* Setup of numbers and text areas ------------------------------------------------------------ */

initializers.push(initTextareas);

keyHandlers.push((e, t) => t.matches('[data-zeyn-number] input') && numberKey(e, t));

clickHandlers.push((e, t) => {
  const b = t.closest('[data-zeyn-number] [data-zeyn-step]');
  if (!b) return false;
  const input = b.closest('[data-zeyn-number]').querySelector('input');
  stepNumber(input, Number(b.dataset.zeynStep));
  input.focus();
  return true;
});

document.addEventListener('input', (e) => {
  const t = e.target;
  if (!(t instanceof Element)) return;
  if (t.matches('[data-zeyn-number] input') && e.inputType) formatNumber(t);
  else if (t.matches('textarea[data-zeyn-autosize]')) textareaInput(t);
});

initTextareas(document);

/* Setup of masked text and passwords --------------------------------------------------------- */

initializers.push(initMasks, initPasswords);

clickHandlers.push((e, t) => {
  const b = t.closest('.z-password-toggle');
  return b && (togglePassword(b), true);
});

document.addEventListener('input', (e) => {
  const t = e.target;
  if (!(t instanceof HTMLInputElement)) return;
  if (t.matches('.z-mask-input')) maskInput(t, e);
  else if (t.matches('[data-zeyn-password] input')) meterInput(t);
});

document.addEventListener('focusin', (e) => {
  const t = e.target;
  if (t instanceof Element && t.matches('.z-mask > .z-native')) t.parentElement.querySelector('.z-mask-input')?.focus();
});

document.addEventListener(
  'submit',
  (e) => {
    for (const b of e.target.querySelectorAll?.('.z-password-toggle[aria-pressed="true"]') ?? []) togglePassword(b);
  },
  true,
);

/** Validation marks set on the raw input show on the masked one. */
new MutationObserver((records) => {
  for (const r of records) {
    const el = r.target;
    if (!(el instanceof Element) || !el.matches('.z-mask > .z-native')) continue;
    const shown = el.parentElement.querySelector('.z-mask-input');
    if (el.getAttribute('aria-invalid') === 'true') shown?.setAttribute('aria-invalid', 'true');
    else shown?.removeAttribute('aria-invalid');
  }
}).observe(document.body, { subtree: true, attributes: true, attributeFilter: ['aria-invalid'] });

initMasks(document);
initPasswords(document);

