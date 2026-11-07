/**
 * Range sliders (`zeyn::ui::RangeSlider`): two native `<input type="range">` handles over one
 * track. Loaded only by pages that hold `[data-zeyn-range]`. The browser moves the handles and
 * sends their values with the form; this module keeps the low handle at or below the high one,
 * moves a handle by a tenth of the range on Page Up and Page Down, writes the formatted values
 * into the label and `aria-valuetext`, fills the track between the handles (`--z-lo`, `--z-hi`)
 * and moves the nearer handle when the bare track is pressed. Date sliders copy each value as an
 * ISO date into their hidden input. The track's box is read once per press, never while a
 * handle moves. Everything is delegated from the document, so sliders that arrive later work too.
 * @module range
 */

/** Groups digits of a whole number with `sep`. */
function grouped(n, sep) {
  const s = String(Math.abs(Math.trunc(n)));
  let out = '';
  for (let i = 0; i < s.length; i++) {
    if (i > 0 && (s.length - i) % 3 === 0) out += sep;
    out += s[i];
  }
  return (n < 0 ? '-' : '') + out;
}

/** The ISO date `days` after 1970-01-01. */
function iso(days) {
  return new Date(days * 86400000).toISOString().slice(0, 10);
}

/** A value as the server wrote it: grouped number, money in whole units, or a date. */
function shown(slider, v) {
  let f = slider.zeynFormat;
  if (!f) {
    try {
      f = JSON.parse(slider.dataset.format ?? '{}') ?? {};
    } catch {
      f = {};
    }
    slider.zeynFormat = f;
  }
  if (f.kind === 'date') {
    const lang = document.documentElement.lang || undefined;
    return new Date(v * 86400000).toLocaleDateString(lang, { timeZone: 'UTC', day: 'numeric', month: 'short', year: 'numeric' });
  }
  if (f.kind === 'money') {
    const plain = `${grouped(v, f.group)}${f.decimal}00`;
    return f.first ? `${f.symbol}${plain}` : `${plain} ${f.symbol}`;
  }
  return grouped(v, f.group ?? ',');
}

/** Writes both values of a slider into its label, track and inputs. */
function paint(slider) {
  const [lo, hi] = slider.querySelectorAll('.z-range-input');
  if (!lo || !hi) return;
  const min = Number(lo.min);
  const span = Number(lo.max) - min || 1;
  const a = Number(lo.value);
  const b = Number(hi.value);
  slider.style.setProperty('--z-lo', `${((a - min) * 100) / span}%`);
  slider.style.setProperty('--z-hi', `${((b - min) * 100) / span}%`);
  const texts = [shown(slider, a), shown(slider, b)];
  [lo, hi].forEach((input, i) => input.setAttribute('aria-valuetext', texts[i]));
  const out = [slider.querySelector('.z-range-lo'), slider.querySelector('.z-range-hi')];
  out.forEach((el, i) => {
    if (el) el.textContent = texts[i];
  });
  const dates = slider.querySelectorAll('.z-range-date');
  if (dates.length === 2) {
    dates[0].value = iso(a);
    dates[1].value = iso(b);
  }
}

/** Keeps the handle that moved on its side of the other one. */
function hold(input) {
  const slider = input.closest('[data-zeyn-range]');
  if (!slider) return;
  const [lo, hi] = slider.querySelectorAll('.z-range-input');
  if (input === lo && Number(lo.value) > Number(hi.value)) lo.value = hi.value;
  if (input === hi && Number(hi.value) < Number(lo.value)) hi.value = lo.value;
  paint(slider);
}

/** Sets a handle as if the user moved it: `input`, then `change` for forms and filters. */
function move(input, value) {
  const before = input.value;
  input.value = String(value);
  hold(input);
  if (input.value === before) return;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
}

document.addEventListener('input', (e) => {
  if (e.target instanceof HTMLInputElement && e.target.matches('[data-zeyn-range] .z-range-input')) hold(e.target);
});

document.addEventListener('keydown', (e) => {
  const input = e.target;
  if (!(input instanceof HTMLInputElement) || !input.matches('[data-zeyn-range] .z-range-input')) return;
  if (e.key !== 'PageUp' && e.key !== 'PageDown') return;
  e.preventDefault();
  const min = Number(input.min);
  const max = Number(input.max);
  const step = Number(input.step) || 1;
  const big = Math.max(step, Math.round((max - min) / 10 / step) * step);
  const next = Number(input.value) + (e.key === 'PageUp' ? big : -big);
  move(input, Math.min(max, Math.max(min, next)));
});

document.addEventListener('pointerdown', (e) => {
  const track = e.target instanceof Element ? e.target.closest('[data-zeyn-range] .z-range-track') : null;
  if (!track || e.target.matches('.z-range-input') || e.button !== 0) return;
  const [lo, hi] = track.querySelectorAll('.z-range-input');
  if (!lo || !hi) return;
  const box = track.getBoundingClientRect();
  const rtl = getComputedStyle(track).direction === 'rtl';
  const share = Math.min(1, Math.max(0, (rtl ? box.right - e.clientX : e.clientX - box.left) / (box.width || 1)));
  const min = Number(lo.min);
  const step = Number(lo.step) || 1;
  const value = min + Math.round((share * (Number(lo.max) - min)) / step) * step;
  const near = Math.abs(value - Number(lo.value));
  const far = Math.abs(value - Number(hi.value));
  const input = near < far || (near === far && value < Number(lo.value)) ? lo : hi;
  move(input, value);
  input.focus({ preventScroll: true });
});
