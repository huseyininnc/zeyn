/**
 * Quantity steppers (`zeyn::ui::QuantityStepper`). Loaded only by pages that hold
 * `[data-zeyn-quantity]`. The minus and plus buttons change the number field at once; 400 ms
 * after the last change the quantity is posted to `POST /_zeyn/data` (key `quantity`) with the
 * CSRF header. The server clamps it to the stepper's range and runs the action, which answers with
 * the quantity it kept and the most allowed (`{quantity, max}`). A newer change aborts the request
 * still running, so only the last answer is applied. When the kept quantity is less than asked,
 * the status line says how many are available; a failed request restores the last kept quantity
 * and says so.
 * @module quantity
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const WAIT = 400;

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/** Parses JSON, or returns `null`. */
function parse(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** The field, the two buttons and the status line of a stepper. */
function parts(box) {
  return {
    input: box.querySelector('.z-qty-input'),
    down: box.querySelector('[data-step="-1"]'),
    up: box.querySelector('[data-step="1"]'),
    status: box.querySelector('.z-qty-status'),
  };
}

/** Keeps the value inside the field's range and turns the buttons off at the ends. */
function bound(box) {
  const { input, down, up } = parts(box);
  const min = Number(input.min);
  const max = Number(input.max);
  let v = Math.round(Number(input.value));
  if (!Number.isFinite(v)) v = min;
  v = Math.min(max, Math.max(min, v));
  if (String(v) !== input.value) input.value = String(v);
  down.disabled = v <= min;
  up.disabled = v >= max;
  return v;
}

/** Sends the quantity after the user stops changing it. */
function later(box) {
  if (box.zeynKept == null) box.zeynKept = Number(box.querySelector('.z-qty-input').defaultValue);
  clearTimeout(box.zeynTimer);
  box.zeynTimer = setTimeout(() => send(box), WAIT);
}

/** Posts the quantity and applies the answer, unless a newer change took over. */
async function send(box) {
  const { input, status } = parts(box);
  const asked = bound(box);
  box.zeynRun?.abort();
  const run = new AbortController();
  box.zeynRun = run;
  const screen = box.closest('[data-zeyn-screen]');
  const body = {
    screen: screen?.dataset.zeynScreen ?? null,
    params: parse(screen?.dataset.zeynParams ?? 'null'),
    node: box.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
    key: 'quantity',
    input: { quantity: asked },
  };
  box.setAttribute('aria-busy', 'true');
  let state = null;
  try {
    const res = await fetch(`${BASE}/_zeyn/data`, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify(body),
      signal: run.signal,
    });
    if (res.ok) state = parse(await res.text());
  } catch {
    if (run.signal.aborted) return;
  }
  if (box.zeynRun !== run) return;
  box.removeAttribute('aria-busy');
  if (!state || !Number.isFinite(Number(state.quantity))) {
    input.value = String(box.zeynKept);
    bound(box);
    status.textContent = box.dataset.textFailed ?? '';
    return;
  }
  const kept = Number(state.quantity);
  box.zeynKept = kept;
  if (Number.isFinite(Number(state.max))) input.max = String(Math.max(Number(input.min), Number(state.max)));
  input.value = String(kept);
  bound(box);
  status.textContent = kept < asked ? (box.dataset.textLimit ?? '').replace('{n}', String(state.max ?? kept)) : '';
}

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('[data-zeyn-quantity] .z-qty-step') : null;
  if (!button || button.disabled) return;
  const box = button.closest('[data-zeyn-quantity]');
  const { input } = parts(box);
  input.value = String(Math.round(Number(input.value) || 0) + Number(button.dataset.step));
  bound(box);
  if (button.disabled) input.focus();
  later(box);
});

document.addEventListener('input', (e) => {
  const input = e.target;
  if (!(input instanceof HTMLInputElement) || !input.matches('[data-zeyn-quantity] .z-qty-input')) return;
  if (input.value.trim() === '') return;
  later(input.closest('[data-zeyn-quantity]'));
});

document.addEventListener('change', (e) => {
  const input = e.target;
  if (!(input instanceof HTMLInputElement) || !input.matches('[data-zeyn-quantity] .z-qty-input')) return;
  const box = input.closest('[data-zeyn-quantity]');
  bound(box);
  later(box);
});
