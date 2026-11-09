/**
 * Drafts of forms (what was typed survives a reload, never masked or secret values) and the
 * workflow transition buttons of forms. Loaded only by pages that hold a form
 * (`form[data-zeyn-submit]`) or a transition button.
 * @module forms
 */
const B = await import(new URL(`enterprise.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { BASE, address, hooks, t } = B;

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('button[data-zeyn-transition]') : null;
  const field = button?.form?.elements.namedItem('transition');
  if (field instanceof HTMLInputElement) field.value = button.dataset.zeynTransition;
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
      !el.matches('[data-zeyn-masked], [type="password"], [autocomplete="one-time-code"], [type="file"], [type="hidden"], button, fieldset, output'),
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
  if (!a.screen || secretForm(form)) return;
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

/** Sign in and code forms keep no drafts at all: nothing of them is worth restoring. */
function secretForm(form) {
  return !!form.querySelector('[type="password"], [autocomplete="one-time-code"]');
}

function saveAllDrafts() {
  document.querySelectorAll('form[data-zeyn-submit].z-dirty').forEach(saveDraft);
}

document.addEventListener('zeyn:save-drafts', saveAllDrafts);

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

