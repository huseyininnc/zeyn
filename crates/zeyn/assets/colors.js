/**
 * Color controls (`Control::Color`): a hex input with a chip of the color and a picker with red,
 * green, blue and alpha sliders, the colors used last and the app's palette. The hex input is
 * what the form submits; the picker writes it and sends `change` when it closes. Loaded only by
 * pages that hold one (`data-zeyn-color`).
 * @module colors
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { clickHandlers, closePopup, initializers, make, openPopup, parse, texts, uid } = K;

const HEX = /^#(?:[0-9a-f]{6}|[0-9a-f]{8})$/i;
const RECENT_KEY = 'zeyn.colors.recent';
const MAX_RECENT = 8;
const CHANNELS = ['red', 'green', 'blue', 'alpha'];

/** Red, green, blue and alpha (0 to 255) of a hex color. */
function channels(hex) {
  const h = hex.slice(1);
  return [0, 2, 4, 6].map((i) => (i < h.length ? parseInt(h.slice(i, i + 2), 16) : 255));
}

/** A hex color from its channels; the alpha is left out when the color is opaque. */
function toHex(c) {
  const h = (v) => v.toString(16).padStart(2, '0').toUpperCase();
  return `#${h(c[0])}${h(c[1])}${h(c[2])}${c[3] === 255 ? '' : h(c[3])}`;
}

/** The colors used last, newest first. Empty when the browser keeps no storage. */
function recent() {
  try {
    const list = parse(localStorage.getItem(RECENT_KEY) ?? '[]');
    return Array.isArray(list) ? list.filter((c) => typeof c === 'string' && HEX.test(c)).slice(0, MAX_RECENT) : [];
  } catch {
    return [];
  }
}

/** Puts a color first in the colors used last. */
function remember(hex) {
  const list = [hex, ...recent().filter((c) => c !== hex)].slice(0, MAX_RECENT);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(list));
  } catch {
    /* no storage: the list is not kept */
  }
}

/** Paints the chip of a color control with the color in its input. */
function paint(wrap) {
  const hex = wrap.querySelector('.z-color-hex').value.trim();
  wrap.querySelector('.z-color-chip').style.setProperty('--z-chip', HEX.test(hex) ? hex : 'transparent');
}

/** Shows the picker buttons, which need this script, and paints the chips. */
function initColors(root) {
  root.querySelectorAll('.z-color:not([data-ready])').forEach((wrap) => {
    wrap.dataset.ready = '';
    const button = wrap.querySelector('.z-color-open');
    const input = wrap.querySelector('.z-color-hex');
    if (button) button.hidden = input.readOnly || input.disabled;
    wrap.zeynSync = () => paint(wrap);
    paint(wrap);
  });
}

/** A group of color buttons: the colors used last or the palette. */
function swatches(label, colors, pick) {
  const group = make('div', 'z-color-swatches', { role: 'group', 'aria-label': label });
  for (const hex of colors) {
    const b = make('button', 'z-color-swatch', { type: 'button', 'aria-label': hex, title: hex, 'data-color': hex });
    b.style.setProperty('--z-chip', hex);
    b.addEventListener('click', () => pick(hex));
    group.append(b);
  }
  return group;
}

/** Opens the picker of a color control (W3C APG dialog: focus moves in, Escape closes). */
function openPicker(wrap) {
  const input = wrap.querySelector('.z-color-hex');
  const button = wrap.querySelector('.z-color-open');
  const t = texts(wrap);
  const before = input.value.trim();
  const color = channels(HEX.test(before) ? before : '#000000');
  const panel = make('div', 'z-color-pop', { role: 'dialog', 'aria-label': t.dialog ?? '' });
  const preview = make('div', 'z-color-preview', { 'aria-hidden': 'true' });
  const sliders = CHANNELS.map((name, i) => {
    const id = `z-color-${uid()}`;
    const row = make('div', 'z-color-row');
    const slider = make('input', 'z-color-slider', { type: 'range', id, min: '0', max: '255', step: '1', value: String(color[i]) });
    row.append(make('label', '', { for: id, text: t[name] ?? name }), slider, make('output', 'z-color-number', { for: id, text: String(color[i]) }));
    slider.addEventListener('input', () => {
      color[i] = Number(slider.value);
      update();
    });
    return row;
  });
  const update = () => {
    const hex = toHex(color);
    input.value = hex;
    preview.style.setProperty('--z-chip', hex);
    sliders.forEach((row, i) => {
      row.querySelector('input').value = String(color[i]);
      row.querySelector('output').textContent = String(color[i]);
    });
    paint(wrap);
  };
  const pick = (hex) => {
    channels(hex).forEach((v, i) => {
      color[i] = v;
    });
    update();
  };
  panel.append(preview, ...sliders);
  const last = recent();
  if (last.length) panel.append(make('p', 'z-color-heading', { text: t.recent ?? '' }), swatches(t.recent ?? '', last, pick));
  const palette = (parse(wrap.dataset.palette ?? '[]') ?? []).filter((c) => typeof c === 'string' && HEX.test(c));
  if (palette.length) panel.append(make('p', 'z-color-heading', { text: t.palette ?? '' }), swatches(t.palette ?? '', palette, pick));
  const done = make('button', 'z-button z-primary z-color-done', { type: 'button', text: t.done ?? 'OK' });
  done.addEventListener('click', () => closePopup(true));
  panel.append(done);
  preview.style.setProperty('--z-chip', toHex(color));
  button.setAttribute('aria-expanded', 'true');
  openPopup(panel, {
    anchor: wrap,
    owner: button,
    trap: true,
    onClose: () => {
      button.setAttribute('aria-expanded', 'false');
      const after = input.value.trim();
      if (after === before) return;
      if (HEX.test(after)) remember(after.toUpperCase());
      input.dispatchEvent(new Event('input', { bubbles: true }));
      input.dispatchEvent(new Event('change', { bubbles: true }));
    },
  });
  sliders[0].querySelector('input').focus();
}

initializers.push(initColors);

clickHandlers.push((e, t) => {
  const b = t.closest('.z-color-open');
  if (!b) return false;
  openPicker(b.closest('.z-color'));
  return true;
});

document.addEventListener('input', (e) => {
  const t = e.target;
  if (t instanceof HTMLInputElement && t.matches('.z-color-hex')) paint(t.closest('.z-color'));
});

document.addEventListener('change', (e) => {
  const t = e.target;
  if (t instanceof HTMLInputElement && t.matches('.z-color-hex') && HEX.test(t.value.trim())) remember(t.value.trim().toUpperCase());
});

initColors(document);
