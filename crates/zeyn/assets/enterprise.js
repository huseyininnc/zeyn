/**
 * Zeyn enterprise runtime.
 *
 * Works next to `zeyn.js` and never changes how it behaves: revealing masked values on request.
 * Every decision is made on the server; this file only asks and shows. Texts come from
 * `meta[name=zeyn-texts]` in the user's language.
 * @module zeyn-enterprise
 */

/** Path the app is served under, from `meta[name=zeyn-base]`. */
const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');

/** Runtime texts in the user's language. */
const TEXTS = (() => {
  try {
    return JSON.parse(document.querySelector('meta[name="zeyn-texts"]')?.content ?? '{}');
  } catch {
    return {};
  }
})();

/** Returns a runtime text, with `{name}` placeholders filled. */
function t(key, values = {}) {
  return (TEXTS[key] ?? key).replace(/\{(\w+)\}/g, (m, k) => (k in values ? String(values[k]) : m));
}

/** Returns the CSRF token from `meta[name=zeyn-csrf]`. */
function csrf() {
  return document.querySelector('meta[name="zeyn-csrf"]')?.content ?? '';
}

/** Screen name, params and node of an element, as the server expects them. */
function address(el) {
  const screen = el.closest('[data-zeyn-screen]');
  let params = null;
  try {
    params = JSON.parse(screen?.dataset.zeynParams || 'null');
  } catch {
    params = null;
  }
  const node = el.closest('[data-zeyn-node]');
  return { screen: screen?.dataset.zeynScreen ?? null, params, node: node ? Number(node.dataset.zeynNode) : null };
}

/**
 * Sends a JSON `POST` with the CSRF header.
 * @returns {Promise<{status: number, ok: boolean, data: any}>}
 */
async function post(path, body) {
  try {
    const res = await fetch(BASE + path, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': csrf() },
      body: JSON.stringify(body),
    });
    let data = null;
    try {
      data = res.status === 204 ? null : await res.json();
    } catch {
      data = null;
    }
    return { status: res.status, ok: res.ok, data };
  } catch {
    return { status: 0, ok: false, data: null };
  }
}

/** Announces a message in the page's polite live region. */
function announce(message, error = false) {
  const region = document.querySelector('dialog[open] .z-toasts') ?? document.querySelector('.z-toasts');
  if (!region) return;
  const el = document.createElement('div');
  el.className = `z-toast z-toast-${error ? 'error' : 'info'}`;
  el.textContent = message;
  region.append(el);
  setTimeout(() => el.remove(), 5000);
}

/**
 * Asks the server for the raw value of a masked field. The server checks the unmask permission
 * and audits the request; the value replaces the masked text only in this page.
 */
async function reveal(button) {
  const field = button.dataset.zeynReveal;
  const r = await post('/_zeyn/reveal', { ...address(button), field, key: button.dataset.zeynKey ?? null });
  if (!r.ok || typeof r.data?.value !== 'string') {
    announce(r.data?.message || t('privacy.reveal_failed'), true);
    return;
  }
  const box = button.closest('.z-masked-field');
  const input = box?.querySelector('input[data-zeyn-masked]');
  if (input) {
    input.value = r.data.value;
    input.readOnly = false;
    input.removeAttribute('data-zeyn-masked');
    button.remove();
    input.focus();
  } else {
    const span = button.previousElementSibling;
    if (span?.matches('[data-zeyn-masked]')) {
      span.textContent = r.data.value;
      span.removeAttribute('data-zeyn-masked');
      span.tabIndex = -1;
      button.remove();
      span.focus();
    }
  }
  announce(t('privacy.revealed'));
}

document.addEventListener('click', (e) => {
  const el = e.target instanceof Element ? e.target.closest('button[data-zeyn-reveal]') : null;
  if (!el) return;
  e.preventDefault();
  e.stopPropagation();
  reveal(el);
});
