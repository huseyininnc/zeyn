/**
 * Zeyn enterprise runtime.
 *
 * Works next to `zeyn.js` and never changes how it behaves. Runs on every page because it is
 * about the page, not a component: response hooks on `fetch` (reasons for opening a screen,
 * signing in again, a new deployed version) and the idle timeout of the session. Every decision
 * is made on the server; this file only asks and shows. Texts come from `meta[name=zeyn-texts]`
 * in the user's language. The parts for components are modules of their own, loaded by pages
 * that hold them: `reveal.js` (masked values), `forms.js` (drafts and workflow transitions) and
 * `import.js` (CSV and XLSX import).
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

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
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
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
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

/** Asks `forms.js` to keep the drafts of the forms on the page before it goes away. */
function saveAllDrafts() {
  document.dispatchEvent(new CustomEvent('zeyn:save-drafts'));
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
 * Sends a reason for opening a screen. Shows the server's message under the field when it is
 * refused.
 * @returns {Promise<boolean>} Whether the server accepted it.
 */
async function sendReason(form, screen) {
  const field = form.elements.namedItem('reason');
  const error = form.querySelector('.z-field-error');
  const reason = field.value.trim();
  if (reason.length < 3 || reason.length > 300) {
    error.textContent = t('privacy.reason_invalid');
    field.setAttribute('aria-invalid', 'true');
    field.focus();
    return false;
  }
  const r = await post('/_zeyn/reason', { screen, reason });
  if (r.ok) return true;
  error.textContent = r.data?.errors?.[0]?.message || r.data?.message || t('privacy.reason_invalid');
  field.setAttribute('aria-invalid', 'true');
  field.focus();
  return false;
}

document.addEventListener('submit', async (e) => {
  const form = e.target;
  if (!(form instanceof HTMLFormElement) || !form.matches('form[data-zeyn-reason]') || form.closest('dialog.z-reason-dialog')) return;
  e.preventDefault();
  if (await sendReason(form, form.dataset.zeynReason) && document.dispatchEvent(new CustomEvent('zeyn:refresh', { cancelable: true }))) location.reload();
});

/**
 * Asks for a reason in a modal dialog.
 * @returns {Promise<boolean>} True once the server accepted a reason, false when cancelled.
 */
function askReason(screen) {
  return new Promise((resolve) => {
    const dialog = document.createElement('dialog');
    dialog.className = 'z-dialog z-reason-dialog';
    dialog.setAttribute('aria-labelledby', 'z-reason-dialog-title');
    dialog.setAttribute('aria-describedby', 'z-reason-dialog-help');
    dialog.innerHTML = `<div class="z-window"><h2 id="z-reason-dialog-title"></h2><p id="z-reason-dialog-help"></p><form class="z-form" data-zeyn-reason novalidate><div class="z-field"><label for="z-reason-dialog-text"></label><textarea id="z-reason-dialog-text" name="reason" required aria-required="true" minlength="3" maxlength="300" rows="3" aria-describedby="z-reason-dialog-help z-reason-dialog-error"></textarea><p class="z-field-error" id="z-reason-dialog-error" aria-live="polite"></p></div><div class="z-form-actions"><button type="button" class="z-button" data-cancel></button><button type="submit" class="z-button z-primary"></button></div></form></div>`;
    dialog.querySelector('h2').textContent = t('privacy.reason_title');
    dialog.querySelector('p').textContent = t('privacy.reason_explain');
    dialog.querySelector('label').textContent = t('privacy.reason_label');
    dialog.querySelector('[data-cancel]').textContent = t('privacy.reason_cancel');
    dialog.querySelector('[type="submit"]').textContent = t('privacy.reason_continue');
    const opener = document.activeElement;
    let accepted = false;
    const form = dialog.querySelector('form');
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      if (await sendReason(form, screen)) {
        accepted = true;
        dialog.close();
      }
    });
    dialog.querySelector('[data-cancel]').addEventListener('click', () => dialog.close());
    dialog.addEventListener('close', () => {
      dialog.remove();
      if (opener instanceof HTMLElement) opener.focus();
      resolve(accepted);
    });
    document.body.append(dialog);
    dialog.showModal();
    dialog.querySelector('textarea').focus();
  });
}

/** Screen a runtime request is about: from `/_zeyn/screen/{name}` or the JSON body. */
function requestScreen(input, init) {
  const url = typeof input === 'string' ? input : input?.url ?? '';
  const m = /\/_zeyn\/screen\/([^/?#]+)/.exec(url);
  if (m) return decodeURIComponent(m[1]);
  try {
    return JSON.parse(init?.body ?? 'null')?.screen ?? null;
  } catch {
    return null;
  }
}

/**
 * Response hooks run on every `fetch` response of the page, including those of `zeyn.js`. A hook
 * may return a replacement response, for example after asking the user for a reason and sending
 * the request again.
 * @type {((res: Response, input: RequestInfo, init?: RequestInit) => Promise<Response | null>)[]}
 */
const hooks = [];
const nativeFetch = window.fetch.bind(window);
window.fetch = async (input, init) => {
  const res = await nativeFetch(input, init);
  for (const hook of hooks) {
    const next = await hook(res, input, init);
    if (next) return next;
  }
  return res;
};

hooks.push(async (res, input, init) => {
  if (res.status !== 428) return null;
  const data = await res.clone().json().catch(() => null);
  const screen = data?.error === 'reason_required' ? requestScreen(input, init) : null;
  if (!screen || !(await askReason(screen))) return null;
  return nativeFetch(input, init);
});

/** Build id of this page, from `meta[name=zeyn-build]`. */
const BUILD = document.querySelector('meta[name="zeyn-build"]')?.content ?? '';
let noticeShown = false;

/**
 * Tells the user that a new version is deployed. It never reloads by itself: the user decides
 * when, and form entries survive the reload as drafts.
 */
function newVersion() {
  if (noticeShown || document.querySelector('meta[name="zeyn-dev"]')) return;
  noticeShown = true;
  const bar = document.createElement('div');
  bar.className = 'z-notice';
  bar.setAttribute('role', 'status');
  const text = document.createElement('p');
  text.textContent = t('deploy.new_version');
  const reload = document.createElement('button');
  reload.type = 'button';
  reload.className = 'z-button z-primary';
  reload.textContent = t('deploy.reload');
  reload.addEventListener('click', () => {
    saveAllDrafts();
    location.reload();
  });
  const later = document.createElement('button');
  later.type = 'button';
  later.className = 'z-button';
  later.textContent = t('deploy.later');
  later.addEventListener('click', () => bar.remove());
  bar.append(text, reload, later);
  document.body.append(bar);
}

hooks.push(async (res) => {
  const build = res.headers.get('x-zeyn-build');
  if (BUILD && build && build !== BUILD) newVersion();
  return null;
});

document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible' && !noticeShown) fetch(`${BASE}/_zeyn/health`, { cache: 'no-store' }).catch(() => {});
});
/** Session settings from `meta[name=zeyn-session]`, present for signed in users. */
const SESSION = (() => {
  try {
    return JSON.parse(document.querySelector('meta[name="zeyn-session"]')?.content ?? 'null');
  } catch {
    return null;
  }
})();

/** Address of a sign in page with `return_to` set to this page. */
function withReturn(url) {
  const sep = url.includes('?') ? '&' : '?';
  return `${url}${sep}return_to=${encodeURIComponent(location.pathname + location.search)}`;
}

/**
 * Builds a modal dialog with a title, a text and buttons. Escape does nothing unless `cancel` is
 * given, so a warning cannot be dismissed by accident.
 * @param {{title: string, body: string, buttons: {label: string, primary?: boolean, run: () => void}[], role?: string, cancel?: () => void}} spec
 */
function modal(spec) {
  const dialog = document.createElement('dialog');
  dialog.className = 'z-dialog z-session-dialog';
  dialog.setAttribute('role', spec.role ?? 'dialog');
  const id = `z-session-${Math.random().toString(36).slice(2)}`;
  dialog.setAttribute('aria-labelledby', `${id}-title`);
  dialog.setAttribute('aria-describedby', `${id}-body`);
  const win = document.createElement('div');
  win.className = 'z-window';
  const h = document.createElement('h2');
  h.id = `${id}-title`;
  h.textContent = spec.title;
  const p = document.createElement('p');
  p.id = `${id}-body`;
  p.textContent = spec.body;
  const actions = document.createElement('div');
  actions.className = 'z-form-actions';
  for (const b of spec.buttons) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = b.primary ? 'z-button z-primary' : 'z-button';
    button.textContent = b.label;
    button.addEventListener('click', b.run);
    actions.append(button);
  }
  win.append(h, p, actions);
  dialog.append(win);
  dialog.addEventListener('cancel', (e) => {
    e.preventDefault();
    spec.cancel?.();
  });
  document.body.append(dialog);
  dialog.showModal();
  (actions.querySelector('.z-primary') ?? actions.querySelector('button'))?.focus();
  return { dialog, body: p };
}

const ACTIVITY_KEY = 'zeyn:activity';
let lastWrite = 0;

/** Last input in any tab of the app, in milliseconds. */
function lastActivity() {
  try {
    return Number(localStorage.getItem(ACTIVITY_KEY)) || Date.now();
  } catch {
    return lastWrite || Date.now();
  }
}

function touch(force = false) {
  const now = Date.now();
  if (!force && now - lastWrite < 5000) return;
  lastWrite = now;
  try {
    localStorage.setItem(ACTIVITY_KEY, String(now));
  } catch {
    return;
  }
}

/**
 * Idle timeout: warns `warn` seconds before the end with a countdown, then sends the user to the
 * sign out or sign in page, or shows a signed out screen. The server's own session rules still
 * apply; this only stops a forgotten screen from staying open.
 */
function startIdle(idle, warn) {
  let warning = null;
  let ended = false;
  touch(true);
  for (const type of ['pointerdown', 'keydown', 'wheel', 'touchstart']) {
    document.addEventListener(type, () => {
      if (!warning && !ended) touch();
    }, { passive: true, capture: true });
  }
  const stay = async () => {
    touch(true);
    warning?.dialog.close();
    warning?.dialog.remove();
    warning = null;
    await post('/_zeyn/keepalive', {});
  };
  const end = () => {
    ended = true;
    saveAllDrafts();
    warning?.dialog.close();
    warning?.dialog.remove();
    warning = null;
    const target = SESSION.logout ?? (SESSION.login ? withReturn(SESSION.login) : null);
    if (target) {
      location.assign(target);
      return;
    }
    modal({
      title: t('session.signed_out_title'),
      body: t('session.signed_out_body'),
      role: 'alertdialog',
      buttons: [{ label: t('session.sign_in'), primary: true, run: () => location.reload() }],
    });
  };
  setInterval(() => {
    if (ended) return;
    const left = Math.ceil((lastActivity() + idle * 1000 - Date.now()) / 1000);
    if (left <= 0) return end();
    if (left > warn) {
      if (warning) {
        warning.dialog.close();
        warning.dialog.remove();
        warning = null;
      }
      return;
    }
    if (!warning) {
      warning = modal({
        title: t('session.idle_title'),
        body: t('session.idle_body', { seconds: left }),
        role: 'alertdialog',
        buttons: [{ label: t('session.stay'), primary: true, run: stay }],
        cancel: stay,
      });
    }
    warning.body.textContent = t('session.idle_body', { seconds: left });
  }, 1000);
}

if (SESSION?.idle > 0) startIdle(SESSION.idle, Math.min(SESSION.warn ?? 60, SESSION.idle));

/**
 * Asks the user to sign in again for an action that needs a recent sign in (or a second factor,
 * for `mfa_required`), then sends the request again. The sign in page opens in a new tab so the
 * work on this page is kept.
 * @param {string} code `reauth_required` or `mfa_required`.
 * @returns {Promise<boolean>} True when the user wants to try again.
 */
function askReauth(code) {
  const mfa = code === 'mfa_required';
  const url = mfa ? SESSION?.mfa : SESSION?.reauth;
  return new Promise((resolve) => {
    const finish = (again) => {
      m.dialog.close();
      m.dialog.remove();
      resolve(again);
    };
    const buttons = [];
    if (url) buttons.push({ label: t(mfa ? 'session.mfa_open' : 'session.reauth_open'), run: () => window.open(withReturn(url), '_blank', 'noopener') });
    buttons.push({ label: t('session.reauth_continue'), primary: true, run: () => finish(true) });
    buttons.push({ label: t('privacy.reason_cancel'), run: () => finish(false) });
    const m = modal({ title: t(mfa ? 'session.mfa_title' : 'session.reauth_title'), body: t(mfa ? 'session.mfa_body' : 'session.reauth_body'), buttons, cancel: () => finish(false) });
  });
}

hooks.push(async (res, input, init) => {
  if (res.status !== 401) return null;
  const data = await res.clone().json().catch(() => null);
  const code = data?.error ?? (typeof data?.type === 'string' ? data.type.split(':').pop() : null);
  if (code !== 'reauth_required' && code !== 'mfa_required') return null;
  let next = res;
  while (next.status === 401 && (await askReauth(code))) next = await nativeFetch(input, init);
  return next;
});


export { BASE, address, announce, hooks, post, t };
