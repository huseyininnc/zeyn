/**
 * Zeyn upload fields.
 *
 * Browser behavior of `Form::file`: files chosen with the native file input or dropped on the
 * zone are sent one by one to `POST /_zeyn/files/{store}` with the CSRF header. Each shows a
 * progress bar and can be canceled while it uploads, retried when it failed and removed when it
 * is done. The field's value, a JSON text in a hidden text input, holds the files the server
 * accepted; the form submits it like any other input.
 *
 * Image previews use object URLs of the chosen file, revoked when the item goes away. Nothing the
 * user picked is ever put into the page as HTML: names are set with `textContent`.
 * Progress, results and errors are announced in the field's polite live region.
 * @module files
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const PREVIEW = /^image\/(png|jpeg|gif|webp)$/;
const ANNOUNCE_STEP = 25;

/** Upload state by item element: the chosen file, the running request and the object URL. */
const items = new WeakMap();

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

/** Fills `{name}` placeholders. */
function fill(template, values) {
  return String(template ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in values ? values[k] : m));
}

/**
 * Size in the page language.
 * @example size(2048) // "2.0 KB" in English, "2,0 KB" in Turkish
 */
function size(bytes) {
  const units = ['B', 'KB', 'MB', 'GB'];
  let v = bytes;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u += 1;
  }
  const lang = document.documentElement.lang || 'en';
  const text = u === 0 ? String(bytes) : v.toLocaleString(lang, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  return `${text} ${units[u]}`;
}

/** The upload root, its texts and limits. */
function field(root) {
  return {
    root,
    store: root.dataset.zeynUpload,
    many: root.dataset.multiple === 'true',
    maxBytes: Number(root.dataset.maxBytes) || 0,
    maxFiles: Number(root.dataset.maxFiles) || 1,
    texts: parse(root.dataset.texts ?? '{}') ?? {},
    input: root.querySelector('.z-upload-input'),
    value: root.querySelector('.z-upload-value'),
    list: root.querySelector('.z-upload-list'),
    status: root.querySelector('.z-upload-status'),
  };
}

/** Says `text` in the field's live region. */
function announce(f, text) {
  f.status.textContent = '';
  requestAnimationFrame(() => (f.status.textContent = text));
}

/** Writes the accepted files into the value input and tells the form it changed. */
function sync(f) {
  const files = [...f.list.querySelectorAll('.z-upload-item[data-id]')].map((li) => parse(li.dataset.file ?? 'null')).filter(Boolean);
  f.value.value = f.many ? JSON.stringify(files) : files[0] ? JSON.stringify(files[0]) : '';
  f.value.dispatchEvent(new Event('input', { bubbles: true }));
  f.value.dispatchEvent(new Event('change', { bubbles: true }));
}

/** A button of an item. */
function button(cls, label, name) {
  const b = document.createElement('button');
  b.type = 'button';
  b.className = `z-button ${cls}`;
  b.textContent = label;
  if (name) b.setAttribute('aria-label', name);
  return b;
}

/** Replaces the buttons of an item. */
function actions(li, ...buttons) {
  li.querySelectorAll('.z-upload-cancel, .z-upload-retry, .z-upload-remove').forEach((b) => b.remove());
  li.append(...buttons);
}

/** Makes a done item: a download link, its size and a remove button. */
function done(f, li, file) {
  li.dataset.id = file.id;
  li.dataset.file = JSON.stringify(file);
  li.dataset.state = 'done';
  li.querySelector('.z-upload-progress')?.remove();
  li.querySelector('.z-upload-error')?.replaceChildren();
  const name = li.querySelector('.z-upload-name');
  const link = document.createElement('a');
  link.className = 'z-upload-name';
  link.href = `${BASE}/_zeyn/files/${encodeURIComponent(f.store)}/${encodeURIComponent(file.id)}`;
  link.setAttribute('download', '');
  link.textContent = file.name;
  name.replaceWith(link);
  actions(li, button('z-upload-remove', f.texts.remove, fill(f.texts.remove_name, { name: file.name })));
}

/** Shows why an item failed, with retry (unless retrying cannot help) and remove. */
function failed(f, li, reason, canRetry) {
  const s = items.get(li);
  li.dataset.state = 'failed';
  li.querySelector('.z-upload-progress')?.remove();
  li.querySelector('.z-upload-error').textContent = reason;
  const name = s?.file.name ?? '';
  const buttons = [button('z-upload-remove', f.texts.remove, fill(f.texts.remove_name, { name }))];
  if (canRetry) buttons.unshift(button('z-upload-retry', f.texts.retry, `${f.texts.retry}: ${name}`));
  const hadFocus = li.contains(document.activeElement);
  actions(li, ...buttons);
  if (hadFocus) li.querySelector('.z-upload-retry, .z-upload-remove')?.focus();
  announce(f, fill(f.texts.failed, { name, reason }));
}

/** Sends the item's file, with progress, and handles the answer. */
async function send(f, li) {
  const s = items.get(li);
  li.dataset.state = 'uploading';
  li.querySelector('.z-upload-error').textContent = '';
  let bar = li.querySelector('.z-upload-progress');
  if (!bar) {
    bar = document.createElement('progress');
    bar.className = 'z-upload-progress';
    bar.max = 100;
    li.querySelector('.z-upload-size').after(bar);
  }
  bar.value = 0;
  bar.setAttribute('aria-label', fill(f.texts.uploading, { name: s.file.name, pct: 0 }));
  const hadFocus = li.contains(document.activeElement);
  actions(li, button('z-upload-cancel', f.texts.cancel, `${f.texts.cancel}: ${s.file.name}`));
  if (hadFocus) li.querySelector('.z-upload-cancel').focus();
  const token = await csrf();
  const xhr = new XMLHttpRequest();
  s.xhr = xhr;
  let said = -1;
  xhr.open('POST', `${BASE}/_zeyn/files/${encodeURIComponent(f.store)}`);
  xhr.setRequestHeader('x-zeyn-csrf', token);
  xhr.setRequestHeader('x-zeyn-file-name', encodeURIComponent(s.file.name));
  xhr.setRequestHeader('content-type', s.file.type || 'application/octet-stream');
  xhr.upload.addEventListener('progress', (e) => {
    if (!e.lengthComputable) return;
    const pct = Math.floor((e.loaded / e.total) * 100);
    bar.value = pct;
    bar.setAttribute('aria-label', fill(f.texts.uploading, { name: s.file.name, pct }));
    const step = Math.floor(pct / ANNOUNCE_STEP) * ANNOUNCE_STEP;
    if (step > said && step < 100) {
      said = step;
      announce(f, fill(f.texts.uploading, { name: s.file.name, pct: step }));
    }
  });
  xhr.addEventListener('load', () => {
    s.xhr = null;
    const body = parse(xhr.responseText);
    if (xhr.status === 201 && body?.id) {
      done(f, li, body);
      sync(f);
      announce(f, fill(f.texts.uploaded, { name: body.name }));
    } else {
      const final = [413, 415, 422].includes(xhr.status) || xhr.status === 403;
      failed(f, li, body?.message ?? String(xhr.status), !final);
    }
  });
  xhr.addEventListener('error', () => {
    s.xhr = null;
    failed(f, li, f.texts.aborted, true);
  });
  xhr.addEventListener('abort', () => {
    s.xhr = null;
    failed(f, li, fill(f.texts.canceled, { name: s.file.name }), true);
  });
  xhr.send(s.file);
}

/** Adds an item for a chosen file, with a preview for images, and starts its upload. */
function add(f, file) {
  const li = document.createElement('li');
  li.className = 'z-upload-item';
  if (PREVIEW.test(file.type)) {
    const img = document.createElement('img');
    img.className = 'z-upload-preview';
    img.alt = '';
    const url = URL.createObjectURL(file);
    img.src = url;
    li.append(img);
    items.set(li, { file, url, xhr: null });
  } else {
    items.set(li, { file, url: null, xhr: null });
  }
  const name = document.createElement('span');
  name.className = 'z-upload-name';
  name.textContent = file.name;
  const sz = document.createElement('span');
  sz.className = 'z-upload-size';
  sz.textContent = size(file.size);
  const error = document.createElement('span');
  error.className = 'z-upload-error';
  li.append(name, sz, error);
  f.list.append(li);
  if (f.maxBytes && file.size > f.maxBytes) {
    failed(f, li, f.texts.too_large, false);
    return;
  }
  send(f, li);
}

/** Removes an item: stops its upload, frees its preview and moves focus to a neighbor. */
function drop(f, li) {
  const s = items.get(li);
  s?.xhr?.abort();
  if (s?.url) URL.revokeObjectURL(s.url);
  const name = s?.file.name ?? parse(li.dataset.file ?? 'null')?.name ?? '';
  const next = li.nextElementSibling ?? li.previousElementSibling;
  const wasDone = li.hasAttribute('data-id');
  li.remove();
  if (wasDone) sync(f);
  (next?.querySelector('button') ?? f.input).focus();
  announce(f, fill(f.texts.removed, { name }));
}

/** Takes files from the picker or a drop: replaces the file of a single field, checks the count. */
function take(f, list) {
  const files = [...list];
  if (!files.length) return;
  if (!f.many) {
    f.list.querySelectorAll('.z-upload-item').forEach((li) => drop(f, li));
    add(f, files[0]);
    return;
  }
  const room = f.maxFiles - f.list.querySelectorAll('.z-upload-item:not([data-state="failed"])').length;
  files.slice(0, Math.max(room, 0)).forEach((file) => add(f, file));
  if (files.length > room) announce(f, f.texts.too_many);
}

/** Links and previews of the files a form opened with. */
function init(scope) {
  scope.querySelectorAll('.z-upload:not([data-ready])').forEach((root) => {
    root.dataset.ready = '';
    root.querySelectorAll('a[data-zeyn-href]').forEach((a) => (a.href = BASE + a.dataset.zeynHref));
    root.querySelectorAll('.z-upload-item[data-id]').forEach((li) => {
      const file = { id: li.dataset.id, name: li.querySelector('.z-upload-name')?.textContent ?? '' };
      const known = parse(root.querySelector('.z-upload-value').value || 'null');
      const found = (Array.isArray(known) ? known : [known]).find((k) => k?.id === file.id);
      li.dataset.file = JSON.stringify(found ?? file);
      li.dataset.state = 'done';
      if (PREVIEW.test(li.dataset.type ?? '')) {
        const img = document.createElement('img');
        img.className = 'z-upload-preview';
        img.alt = '';
        img.src = li.querySelector('a').href;
        li.prepend(img);
      }
    });
  });
}

document.addEventListener('change', (e) => {
  const input = e.target.closest?.('.z-upload-input');
  if (!input) return;
  take(field(input.closest('.z-upload')), input.files);
  input.value = '';
});

document.addEventListener('click', (e) => {
  const b = e.target.closest?.('.z-upload-cancel, .z-upload-retry, .z-upload-remove');
  if (!b) return;
  const f = field(b.closest('.z-upload'));
  const li = b.closest('.z-upload-item');
  if (b.classList.contains('z-upload-cancel')) items.get(li)?.xhr?.abort();
  else if (b.classList.contains('z-upload-retry')) send(f, li);
  else drop(f, li);
});

for (const type of ['dragenter', 'dragover']) {
  document.addEventListener(type, (e) => {
    const zone = e.target.closest?.('.z-upload-drop');
    if (!zone || !e.dataTransfer?.types.includes('Files')) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
    zone.classList.add('z-dragging');
  });
}

document.addEventListener('dragleave', (e) => {
  const zone = e.target.closest?.('.z-upload-drop');
  if (zone && !zone.contains(e.relatedTarget)) zone.classList.remove('z-dragging');
});

document.addEventListener('drop', (e) => {
  const zone = e.target.closest?.('.z-upload-drop');
  if (!zone) return;
  e.preventDefault();
  zone.classList.remove('z-dragging');
  take(field(zone.closest('.z-upload')), e.dataTransfer?.files ?? []);
});

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));

new MutationObserver((records) => {
  for (const r of records) for (const n of r.addedNodes) if (n instanceof HTMLDialogElement) init(n);
}).observe(document.body, { childList: true });

init(document);
