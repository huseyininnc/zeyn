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
  if (await sendReason(form, form.dataset.zeynReason)) location.reload();
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
  if (noticeShown) return;
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
