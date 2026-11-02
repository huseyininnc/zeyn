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

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('button[data-zeyn-transition]') : null;
  const field = button?.form?.elements.namedItem('transition');
  if (field instanceof HTMLInputElement) field.value = button.dataset.zeynTransition;
});

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

const DRAFT_PREFIX = 'zeyn-draft:';
const DRAFT_DAY_MS = 24 * 60 * 60 * 1000;

/** Storage key of a form's draft: base path, screen, params and node. */
function draftKey(a) {
  return `${DRAFT_PREFIX}${BASE}|${a.screen}|${JSON.stringify(a.params ?? null)}|${a.node}`;
}

/** Inputs a draft keeps: what the user typed, never masked, secret or hidden values. */
function draftFields(form) {
  return [...form.elements].filter(
    (el) =>
      el.name &&
      !el.disabled &&
      !el.readOnly &&
      !el.hidden &&
      !el.matches('[data-zeyn-masked], [type="password"], [type="file"], [type="hidden"], button, fieldset, output'),
  );
}

/** Reads one input as a draft value. */
function fieldValue(el) {
  if (el.type === 'checkbox') return el.checked;
  if (el.type === 'radio') return el.checked ? el.value : undefined;
  return el.value;
}

/** Writes one draft value back into an input. Returns true when it changed the input. */
function setField(el, value) {
  if (value === undefined) return false;
  if (el.type === 'checkbox') {
    if (el.checked === value) return false;
    el.checked = value;
  } else if (el.type === 'radio') {
    if (el.checked === (el.value === value)) return false;
    el.checked = el.value === value;
  } else {
    if (el.value === value) return false;
    el.value = value;
  }
  return true;
}

/** Keeps what was typed in a form in `sessionStorage`, with the row version it was typed on. */
function saveDraft(form) {
  const a = address(form);
  if (!a.screen) return;
  const values = {};
  for (const el of draftFields(form)) {
    const v = fieldValue(el);
    if (v !== undefined) values[el.name] = v;
  }
  const version = form.querySelector('input[data-zeyn-version]')?.value ?? null;
  try {
    sessionStorage.setItem(draftKey(a), JSON.stringify({ at: Date.now(), version, values }));
  } catch {
    return;
  }
}

function saveAllDrafts() {
  document.querySelectorAll('form[data-zeyn-submit].z-dirty').forEach(saveDraft);
}

/**
 * Puts a draft back into a form after a reload. The row version of the draft is restored too, so
 * saving a draft of a record someone else changed in the meantime is refused as a conflict.
 */
function restoreDraft(form) {
  if (form.dataset.zeynDraftChecked) return;
  form.dataset.zeynDraftChecked = '1';
  const a = address(form);
  if (!a.screen) return;
  let draft = null;
  try {
    draft = JSON.parse(sessionStorage.getItem(draftKey(a)) ?? 'null');
  } catch {
    draft = null;
  }
  if (!draft?.values) return;
  if (Date.now() - (draft.at ?? 0) > DRAFT_DAY_MS) {
    sessionStorage.removeItem(draftKey(a));
    return;
  }
  const fields = draftFields(form);
  const before = new Map(fields.map((el) => [el, fieldValue(el)]));
  let changed = false;
  for (const el of fields) changed = setField(el, draft.values[el.name]) || changed;
  if (!changed) return;
  const versionInput = form.querySelector('input[data-zeyn-version]');
  const originalVersion = versionInput?.value;
  if (versionInput && draft.version != null) versionInput.value = draft.version;
  form.classList.add('z-dirty');
  const note = document.createElement('div');
  note.className = 'z-draft-note';
  note.setAttribute('role', 'status');
  const text = document.createElement('span');
  text.textContent = t('draft.restored');
  const discard = document.createElement('button');
  discard.type = 'button';
  discard.className = 'z-button';
  discard.textContent = t('draft.discard');
  discard.addEventListener('click', () => {
    for (const [el, v] of before) setField(el, v);
    if (versionInput) versionInput.value = originalVersion;
    sessionStorage.removeItem(draftKey(a));
    form.classList.remove('z-dirty');
    note.remove();
    form.querySelector('input:not([type="hidden"]):not([readonly]), select, textarea')?.focus();
  });
  note.append(text, discard);
  form.prepend(note);
}

let draftTimer = 0;
document.addEventListener('input', (e) => {
  const form = e.target instanceof Element ? e.target.closest('form[data-zeyn-submit]') : null;
  if (!form) return;
  form.classList.add('z-dirty');
  clearTimeout(draftTimer);
  draftTimer = setTimeout(() => saveDraft(form), 300);
});
document.addEventListener('change', (e) => {
  const form = e.target instanceof Element ? e.target.closest('form[data-zeyn-submit]') : null;
  if (form) {
    form.classList.add('z-dirty');
    saveDraft(form);
  }
});
window.addEventListener('pagehide', saveAllDrafts);

hooks.push(async (res, input, init) => {
  const url = typeof input === 'string' ? input : input?.url ?? '';
  if (!res.ok || !url.endsWith('/_zeyn/form')) return null;
  try {
    const body = JSON.parse(init?.body ?? 'null');
    sessionStorage.removeItem(draftKey(body));
    document.querySelectorAll('form[data-zeyn-submit].z-dirty').forEach((f) => {
      const a = address(f);
      if (draftKey(a) === draftKey(body)) {
        f.classList.remove('z-dirty');
        f.querySelector('.z-draft-note')?.remove();
      }
    });
  } catch {
    return null;
  }
  return null;
});

document.querySelectorAll('form[data-zeyn-submit]').forEach(restoreDraft);
new MutationObserver((records) => {
  for (const r of records) {
    for (const n of r.addedNodes) {
      if (!(n instanceof Element)) continue;
      if (n.matches('form[data-zeyn-submit]')) restoreDraft(n);
      n.querySelectorAll('form[data-zeyn-submit]').forEach(restoreDraft);
    }
  }
}).observe(document.body, { childList: true, subtree: true });

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
 * Asks the user to sign in again for an action that needs a recent sign in, then sends the
 * request again. The sign in page opens in a new tab so the work on this page is kept.
 * @returns {Promise<boolean>} True when the user wants to try again.
 */
function askReauth() {
  return new Promise((resolve) => {
    const finish = (again) => {
      m.dialog.close();
      m.dialog.remove();
      resolve(again);
    };
    const buttons = [];
    if (SESSION?.reauth) buttons.push({ label: t('session.reauth_open'), run: () => window.open(withReturn(SESSION.reauth), '_blank', 'noopener') });
    buttons.push({ label: t('session.reauth_continue'), primary: true, run: () => finish(true) });
    buttons.push({ label: t('privacy.reason_cancel'), run: () => finish(false) });
    const m = modal({ title: t('session.reauth_title'), body: t('session.reauth_body'), buttons, cancel: () => finish(false) });
  });
}

hooks.push(async (res, input, init) => {
  if (res.status !== 401) return null;
  const data = await res.clone().json().catch(() => null);
  const code = data?.error ?? (typeof data?.type === 'string' ? data.type.split(':').pop() : null);
  if (code !== 'reauth_required') return null;
  let next = res;
  while (next.status === 401 && (await askReauth())) next = await nativeFetch(input, init);
  return next;
});

const MAX_CSV_BYTES = 8 * 1024 * 1024;

/**
 * Reads a CSV file as text: UTF-8 when it is valid UTF-8, otherwise Windows-1254, the code page
 * spreadsheet programs often save Turkish text in.
 */
async function readCsv(file) {
  const bytes = await file.arrayBuffer();
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    return new TextDecoder('windows-1254').decode(bytes);
  }
}

/** State of each import component: the file text sent for preview, then for import. */
const imports = new WeakMap();

/** Shows the server's answer to a preview or an import. */
function showImport(box, r) {
  const status = box.querySelector('[data-zeyn-import-status]');
  const preview = box.querySelector('[data-zeyn-import-preview]');
  const commit = box.querySelector('[data-zeyn-import-commit]');
  const report = box.querySelector('[data-zeyn-import-report]');
  const data = r.data ?? {};
  commit.disabled = !(r.ok && data.can_import);
  report.hidden = !(r.ok && data.report);
  if (!r.ok) {
    status.textContent = data.errors?.map((e) => e.message).join(' ') || data.message || '';
    status.classList.add('z-import-error');
    preview.innerHTML = '';
    return;
  }
  status.classList.toggle('z-import-error', data.invalid > 0);
  status.textContent = data.summary ?? data.message ?? '';
  preview.innerHTML = data.preview ?? '';
  if (data.errors?.length) {
    const list = document.createElement('ul');
    list.className = 'z-import-errors';
    for (const e of data.errors) {
      const li = document.createElement('li');
      li.textContent = `${e.row}${e.field ? ` / ${e.field}` : ''}: ${e.message}`;
      list.append(li);
    }
    preview.append(list);
  }
  const state = imports.get(box) ?? {};
  state.report = data.report ?? null;
  imports.set(box, state);
}

document.addEventListener('change', async (e) => {
  const input = e.target;
  const box = input instanceof HTMLInputElement && input.type === 'file' ? input.closest('[data-zeyn-import]') : null;
  const file = box && input.files?.[0];
  if (!file) return;
  const status = box.querySelector('[data-zeyn-import-status]');
  if (file.size > MAX_CSV_BYTES) {
    status.textContent = t('import.too_large');
    return;
  }
  status.textContent = t('import.reading');
  const csv = await readCsv(file);
  imports.set(box, { csv, name: file.name.replace(/\.[^.]*$/, '') });
  showImport(box, await post('/_zeyn/import', { ...address(box), csv, commit: false }));
});

document.addEventListener('click', async (e) => {
  const el = e.target instanceof Element ? e.target.closest('[data-zeyn-import-commit], [data-zeyn-import-report]') : null;
  const box = el?.closest('[data-zeyn-import]');
  const state = box && imports.get(box);
  if (!state) return;
  if (el.matches('[data-zeyn-import-report]')) {
    if (!state.report) return;
    const url = URL.createObjectURL(new Blob([state.report], { type: 'text/csv;charset=utf-8' }));
    const a = document.createElement('a');
    a.href = url;
    a.download = `${state.name || 'import'}-errors.csv`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    return;
  }
  el.disabled = true;
  const r = await post('/_zeyn/import', { ...address(box), csv: state.csv, commit: true });
  showImport(box, r);
  if (r.ok) {
    box.querySelector('input[type="file"]').value = '';
    imports.delete(box);
    box.querySelector('[data-zeyn-import-report]').hidden = true;
    announce(r.data?.message ?? '');
  }
});
