/**
 * zeyn.state: the one place the browser keeps client preferences (`zeyn::ClientPref`).
 *
 * Each value is a record `{v, exp, val}` under `zeyn.v1.{key}`. Reads check the record: its
 * version, a value that is `true` or a text of at most 200 characters, and an end time that has
 * not passed and lies no further ahead than the key's lifetime. A record that fails is removed.
 * Necessary keys go to `localStorage` for their lifetime. Preference keys go to `sessionStorage`
 * until the app's consent hook answers yes at `GET /_zeyn/prefs`, then to `localStorage`. When
 * storage is blocked (private windows, strict settings) the values live in memory for the page.
 * Other tabs hear about changes through the `storage` event (`on`). The server never sees any of
 * it. `zeyn.prefs.clear()`, or a press on an element with `data-zeyn-prefs-clear`, removes all.
 * @module state
 * @example
 * const state = await import('/_zeyn/state.js');
 * const key = { name: 'video-notice', ms: 86400000, purpose: 'necessary' };
 * await state.set(key, true);
 * state.get(key); // true
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const PREFIX = 'zeyn.v1.';
const VERSION = 1;
const MAX_TEXT = 200;
const SLACK_MS = 60000;

/** Values kept here when storage is blocked. */
const memory = new Map();
/** @type {Set<(name: string | null) => void>} */
const listeners = new Set();
/** @type {Promise<boolean> | null} */
let consent = null;

/** A storage area that works, or `null` when it is blocked or missing. */
function area(name) {
  try {
    const s = window[name];
    s.setItem('zeyn.probe', '1');
    s.removeItem('zeyn.probe');
    return s;
  } catch {
    return null;
  }
}

const local = area('localStorage');
const session = area('sessionStorage');

/**
 * The key an element is bound to: `data-zeyn-remember`, `-for` (lifetime in ms) and `-purpose`.
 * `null` when the element carries none or the name is not usable.
 */
export function keyOf(el) {
  const name = el?.dataset?.zeynRemember;
  const ms = Number(el?.dataset?.zeynRememberFor);
  if (!name || !/^[A-Za-z0-9_-]{1,40}$/.test(name) || !(ms > 0)) return null;
  return { name, ms, purpose: el.dataset.zeynRememberPurpose === 'preference' ? 'preference' : 'necessary' };
}

/** The value of a stored record when it passes the checks, else `undefined`. */
function check(raw, ms) {
  let r;
  try {
    r = JSON.parse(raw);
  } catch {
    return undefined;
  }
  if (!r || typeof r !== 'object' || r.v !== VERSION || !Number.isInteger(r.exp)) return undefined;
  const now = Date.now();
  if (r.exp <= now || r.exp > now + ms + SLACK_MS) return undefined;
  if (r.val === true) return true;
  if (typeof r.val === 'string' && r.val.length > 0 && r.val.length <= MAX_TEXT) return r.val;
  return undefined;
}

/** Reads a key: memory, then the session, then local storage. Bad records are removed. */
export function get(key) {
  const id = PREFIX + key.name;
  if (memory.has(id)) {
    const v = check(memory.get(id), key.ms);
    if (v !== undefined) return v;
    memory.delete(id);
  }
  for (const s of [session, local]) {
    let raw = null;
    try {
      raw = s?.getItem(id) ?? null;
    } catch {
      continue;
    }
    if (raw === null) continue;
    const v = check(raw, key.ms);
    if (v !== undefined) return v;
    try {
      s.removeItem(id);
    } catch {}
  }
  return undefined;
}

/** Whether preference values may outlive the session, asked once per page. */
function allowed() {
  consent ??= fetch(`${BASE}/_zeyn/prefs`, { credentials: 'same-origin', cache: 'no-store' })
    .then((r) => (r.ok ? r.json() : null))
    .then((b) => b?.preferences === true)
    .catch(() => false);
  return consent;
}

/** Stores `val` (`true` or a short text) for the key's lifetime. */
export async function set(key, val) {
  if (val !== true && !(typeof val === 'string' && val.length > 0 && val.length <= MAX_TEXT)) return;
  const id = PREFIX + key.name;
  const raw = JSON.stringify({ v: VERSION, exp: Date.now() + key.ms, val });
  const lasting = key.purpose === 'necessary' || (await allowed());
  const target = lasting ? local : session;
  const other = lasting ? session : local;
  try {
    if (!target) throw new Error('blocked');
    target.setItem(id, raw);
    other?.removeItem(id);
    memory.delete(id);
  } catch {
    memory.set(id, raw);
  }
}

/** Removes a key everywhere. */
export function remove(key) {
  const id = PREFIX + key.name;
  memory.delete(id);
  for (const s of [session, local]) {
    try {
      s?.removeItem(id);
    } catch {}
  }
}

/** Removes every client preference of the app in this browser. */
export function clear() {
  memory.clear();
  for (const s of [session, local]) {
    try {
      for (let i = (s?.length ?? 0) - 1; i >= 0; i--) {
        const k = s.key(i);
        if (k?.startsWith(PREFIX)) s.removeItem(k);
      }
    } catch {}
  }
  for (const fn of listeners) fn(null);
}

/** Calls `fn(name)` when another tab changes a key (`null`: everything was cleared). */
export function on(fn) {
  listeners.add(fn);
}

window.addEventListener('storage', (e) => {
  if (e.key !== null && !e.key.startsWith(PREFIX)) return;
  const name = e.key === null ? null : e.key.slice(PREFIX.length);
  for (const fn of listeners) fn(name);
});

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('[data-zeyn-prefs-clear]') : null;
  if (button) clear();
});

globalThis.zeyn = Object.assign(globalThis.zeyn ?? {}, { state: { get, set, remove, keyOf, on }, prefs: { clear } });
