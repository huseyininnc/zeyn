/**
 * Inbox bulk actions and the notification bell (`zeyn::ui::Inbox`, `NotificationCenter`).
 * Loaded only by pages that hold `[data-zeyn-inbox]` or `[data-zeyn-notify]`.
 *
 * Inbox: the check boxes choose notifications, the toolbar buttons are enabled once one is
 * chosen and run their action through `POST /_zeyn/data` with the chosen keys. After a success
 * the inbox names its topics (`zeyn:topics`), so it and every listener refresh in place through
 * `topics.js`; a failure is announced and the choice stays.
 *
 * Bell: loads the unread count through `POST /_zeyn/data` after the page shows and again when
 * one of its topics is named (a bulk action, `Effect::refresh`, or the realtime hub through
 * `live.js`), at most once per 300 ms, and writes it into the badge and the link's name.
 * @module inbox
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');

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

/** Calls data source `key` of the component holding `el`: `{ ok, body }`. */
async function data(el, key, input) {
  const screen = el.closest('[data-zeyn-screen]');
  const body = {
    screen: screen?.dataset.zeynScreen ?? null,
    params: parse(screen?.dataset.zeynParams ?? 'null'),
    node: el.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
    key,
    input,
  };
  try {
    const res = await fetch(BASE + '/_zeyn/data', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify(body),
    });
    return { ok: res.ok, body: parse(await res.text()) };
  } catch {
    return { ok: false, body: null };
  }
}

/** Shows a toast through zeyn.js. */
function toast(message, tone) {
  document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects: [{ effect: 'toast', message, tone }] } }));
}

/** Texts of a component, from `data-texts`. */
function texts(el) {
  el.zeynTexts ??= parse(el.dataset.texts ?? '{}') ?? {};
  return el.zeynTexts;
}

/** Enables the buttons of an inbox when something is chosen and keeps "select all" in step. */
function sync(inbox) {
  const boxes = [...inbox.querySelectorAll('.z-inbox-check')];
  const chosen = boxes.filter((b) => b.checked).length;
  for (const b of inbox.querySelectorAll('[data-zeyn-inbox-run]')) b.setAttribute('aria-disabled', String(chosen === 0));
  const all = inbox.querySelector('.z-inbox-check-all');
  if (all) {
    all.checked = chosen > 0 && chosen === boxes.length;
    all.indeterminate = chosen > 0 && chosen < boxes.length;
  }
  const status = inbox.querySelector('[data-zeyn-inbox-status]');
  if (status) status.textContent = chosen ? (texts(inbox).selected ?? '').replace('{n}', chosen) : '';
}

/** Runs a bulk button. */
async function run(button) {
  const inbox = button.closest('[data-zeyn-inbox]');
  if (!inbox || button.getAttribute('aria-disabled') === 'true' || button.dataset.busy) return;
  const ids = [...inbox.querySelectorAll('.z-inbox-check:checked')].map((b) => b.value);
  if (!ids.length) return;
  button.dataset.busy = '';
  inbox.setAttribute('aria-busy', 'true');
  const r = await data(inbox, button.dataset.zeynInboxRun, { ids });
  delete button.dataset.busy;
  inbox.removeAttribute('aria-busy');
  if (!r.ok) {
    toast(r.body?.message || texts(inbox).failed, 'danger');
    return;
  }
  toast((texts(inbox).done ?? '').replace('{n}', r.body?.done ?? ids.length), 'success');
  document.dispatchEvent(new CustomEvent('zeyn:topics', { detail: { topics: (inbox.dataset.topics ?? '').split(',').filter(Boolean) } }));
}

/** Loads the unread count of a bell. */
async function count(bell) {
  const r = await data(bell, 'count', null);
  if (!r.ok || typeof r.body?.unread !== 'number') return;
  const n = r.body.unread;
  const t = texts(bell);
  const badge = bell.querySelector('.z-notify-count');
  const shown = n > 99 ? '99+' : String(n);
  if (badge) {
    badge.textContent = shown;
    badge.hidden = n === 0;
  }
  bell.setAttribute('aria-label', n === 0 ? t.none : (n > 99 ? t.many : t.some).replace('{n}', shown));
}

const timers = new WeakMap();

/** Loads the count of a bell again, once for names that arrive within 300 ms. */
function later(bell) {
  clearTimeout(timers.get(bell));
  timers.set(bell, setTimeout(() => count(bell), 300));
}

/** Sets up the bells inside `root`. */
function init(root) {
  root.querySelectorAll?.('[data-zeyn-notify]:not([data-ready])').forEach((bell) => {
    bell.dataset.ready = '';
    count(bell);
  });
}

document.addEventListener('change', (e) => {
  const box = e.target instanceof Element ? e.target : null;
  const inbox = box?.closest('[data-zeyn-inbox]');
  if (!inbox) return;
  if (box.matches('.z-inbox-check-all')) for (const b of inbox.querySelectorAll('.z-inbox-check')) b.checked = box.checked;
  if (box.matches('.z-inbox-check, .z-inbox-check-all')) sync(inbox);
});

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('[data-zeyn-inbox-run]') : null;
  if (button) run(button);
});

document.addEventListener('zeyn:topics', (e) => {
  const named = new Set(e.detail?.topics ?? []);
  for (const bell of document.querySelectorAll('[data-zeyn-notify]')) if (bell.dataset.zeynNotify.split(',').some((t) => named.has(t))) later(bell);
});

document.addEventListener('zeyn:effect', (e) => {
  const effect = e.detail?.effect;
  if (effect?.effect === 'refresh') document.dispatchEvent(new CustomEvent('zeyn:topics', { detail: { topics: effect.topics } }));
});

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
