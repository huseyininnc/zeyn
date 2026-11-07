/**
 * Reaction buttons (`zeyn::ui::Reactions`). components.js loads this module the first time a
 * page holds them. Pressing a button posts `{reaction, pressed}` to `POST /_zeyn/data` (key
 * `react`) with the CSRF header; the server opens the screen, checks the action's permission
 * and runs it with the item the screen set. The answer is the new state: every count and
 * `aria-pressed` of the row is written from it. A failed press changes nothing and says so in a
 * toast. A row of a feed post (`data-zeyn-post`) also sends the key of its post, which the
 * server checks against the posts the feed's action returns to the user.
 * @module reactions
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

/** Writes a reaction state (`{counts, mine}`) into a row of buttons. */
function apply(row, state) {
  const counts = state?.counts ?? {};
  const mine = Array.isArray(state?.mine) ? state.mine : [];
  for (const button of row.querySelectorAll('button[data-reaction]')) {
    const key = button.dataset.reaction;
    const count = button.querySelector('.z-reaction-count');
    if (count) count.textContent = String(Number(counts[key]) || 0);
    button.setAttribute('aria-pressed', String(mine.includes(key)));
  }
}

/** Sends one press and applies the answer. */
async function press(button) {
  const row = button.closest('[data-zeyn-reactions]');
  if (!row || row.getAttribute('aria-busy') === 'true') return;
  const screen = row.closest('[data-zeyn-screen]');
  const body = {
    screen: screen?.dataset.zeynScreen ?? null,
    params: parse(screen?.dataset.zeynParams ?? 'null'),
    node: row.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
    key: 'react',
    input: { reaction: button.dataset.reaction, pressed: button.getAttribute('aria-pressed') !== 'true', ...(row.hasAttribute('data-zeyn-post') ? { key: row.closest('[data-zeyn-key]')?.dataset.zeynKey ?? '' } : {}) },
  };
  row.setAttribute('aria-busy', 'true');
  let state = null;
  try {
    const res = await fetch(`${BASE}/_zeyn/data`, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify(body),
    });
    if (res.ok) state = parse(await res.text());
  } catch {
    state = null;
  }
  row.removeAttribute('aria-busy');
  if (state) apply(row, state);
  else document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { error: row.dataset.failed } }));
}

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('[data-zeyn-reactions] button[data-reaction]') : null;
  if (button) press(button);
});
