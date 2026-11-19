/**
 * Requests of component modules: the app's base path, the CSRF token of the page and JSON posts,
 * shared by the modules that import it instead of keeping copies of their own. Every request
 * stays on the page's origin (`credentials: 'same-origin'`) and carries the `x-zeyn-csrf` header.
 * @module net
 */

/** The base path the app is mounted at, without a trailing slash. */
export const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
export async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/** Parses JSON, or returns `null`. */
export function parse(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/**
 * Posts JSON to `path` under the base path with the CSRF header. Resolves to `{ ok, body }`,
 * `body` the parsed answer or `null`; a network failure gives `{ ok: false, body: null }`.
 * @example
 * const r = await post('/_zeyn/data', { screen, key: 'move' });
 */
export async function post(path, body) {
  try {
    const res = await fetch(BASE + path, {
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
