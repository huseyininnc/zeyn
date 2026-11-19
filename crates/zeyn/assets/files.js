/**
 * Zeyn upload fields.
 *
 * Browser behavior of `Form::file`: files chosen with the native file input, dropped on the zone
 * (folders too, read through `webkitGetAsEntry`) or pasted while the field has focus are sent to
 * `POST /_zeyn/files/{store}` with the CSRF header, at most `PARALLEL` at a time while the rest
 * wait. A file whose name or type the input's `accept` list does not cover is refused before it is
 * sent; one inside a dropped folder is skipped. Each shows a progress bar and can be canceled
 * while it uploads, retried when it failed and removed when it is done. The field's value, a JSON text in a hidden text input, holds the files the server
 * accepted; the form submits it like any other input.
 *
 * With the `resumable` feature (`data-resumable` on the field) files go through the resumable
 * protocol under `/_zeyn/uploads/` instead: after a lost connection the upload asks the server how
 * much arrived and sends only the rest, retrying with growing delays.
 *
 * Inside a `FileDrop` with an upload action (`[data-zeyn-file-drop][data-save]`), each stored
 * file is then posted to the drop's `upload` source, which runs the action; a refusal shows on
 * the item like a failed upload.
 *
 * Image previews use object URLs of the chosen file, revoked when the item goes away. Nothing the
 * user picked is ever put into the page as HTML: names are set with `textContent`.
 * Progress, results and errors are announced in the field's polite live region.
 * @module files
 */

/** The shared request helpers (`net.js`). */
const { BASE, csrf, parse, post } = await import(new URL(`net.js${new URL(import.meta.url).search}`, import.meta.url).href);
const PREVIEW = /^image\/(png|jpeg|gif|webp)$/;
const ANNOUNCE_STEP = 25;
/** Uploads of one field that run at the same time; the others wait for a free slot. */
const PARALLEL = 3;
/** Entries a dropped folder tree is read for at most, so a huge tree cannot stall the page. */
const MAX_ENTRIES = 10000;
/** Delays before the retries of a resumable upload, in milliseconds; the last one repeats. */
const BACKOFF = [1000, 2000, 4000, 8000, 15000];
/** Failed attempts in a row after which a resumable upload gives up and offers Retry. */
const RETRIES = 12;
/**
 * Bytes of one `PATCH` of a resumable upload: 1 MiB, the default body limit of common reverse
 * proxies, and little to send again when a connection breaks.
 */
const CHUNK = 1024 * 1024;

/** Upload state by item element: the chosen file, the running request and the object URL. */
const items = new WeakMap();

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

/**
 * Whether the input's `accept` list covers `file`, by extension or by content type.
 * @example allowed(f, new File([], 'a.png', { type: 'image/png' })) // true for ".png,image/png"
 */
function allowed(f, file) {
  const list = (f.input.accept || '')
    .toLowerCase()
    .split(',')
    .map((a) => a.trim())
    .filter(Boolean);
  const name = file.name.toLowerCase();
  const type = (file.type || '').toLowerCase();
  return !list.length || list.some((a) => (a.startsWith('.') ? name.endsWith(a) : a.endsWith('/*') ? type.startsWith(a.slice(0, -1)) : type === a));
}

/** Starts waiting uploads of the field while fewer than `PARALLEL` run. */
function pump(f) {
  let running = f.list.querySelectorAll('.z-upload-item[data-state="uploading"]').length;
  for (const li of f.list.querySelectorAll('.z-upload-item[data-state="queued"]')) {
    if (running >= PARALLEL) return;
    running += 1;
    send(f, li);
  }
}

/** Puts an item in line for upload. */
function queue(f, li) {
  li.dataset.state = 'queued';
  li.querySelector('.z-upload-error').textContent = '';
  const name = items.get(li)?.file.name ?? '';
  let bar = li.querySelector('.z-upload-progress');
  if (!bar) {
    bar = document.createElement('progress');
    bar.className = 'z-upload-progress';
    bar.max = 100;
    li.querySelector('.z-upload-size').after(bar);
  }
  bar.removeAttribute('value');
  bar.setAttribute('aria-label', fill(f.texts.waiting, { name }));
  const hadFocus = li.contains(document.activeElement);
  actions(li, button('z-upload-remove', f.texts.remove, fill(f.texts.remove_name, { name })));
  if (hadFocus) li.querySelector('.z-upload-remove').focus();
  pump(f);
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
  let said = -1;
  const progress = (loaded) => {
    const pct = Math.min(100, Math.floor((loaded / Math.max(s.file.size, 1)) * 100));
    bar.value = pct;
    bar.setAttribute('aria-label', fill(f.texts.uploading, { name: s.file.name, pct }));
    const step = Math.floor(pct / ANNOUNCE_STEP) * ANNOUNCE_STEP;
    if (step > said && step < 100) {
      said = step;
      announce(f, fill(f.texts.uploading, { name: s.file.name, pct: step }));
    }
  };
  const finish = (status, text) => {
    queueMicrotask(() => pump(field(f.root)));
    const body = parse(text);
    if ((status === 201 || status === 200) && body?.id) {
      done(f, li, body);
      sync(f);
      announce(f, fill(f.texts.uploaded, { name: body.name }));
      report(f, li, body);
    } else if (status === -1) {
      failed(f, li, fill(f.texts.canceled, { name: s.file.name }), true);
    } else if (status === 0) {
      failed(f, li, f.texts.aborted, true);
    } else {
      const final = [413, 415, 422].includes(status) || status === 403;
      failed(f, li, body?.message ?? String(status), !final);
    }
  };
  s.canceled = false;
  if (f.root.hasAttribute('data-resumable')) {
    const r = await resumable(f, s, token, progress);
    finish(r.status, r.text);
    return;
  }
  const r = await call(s, 'POST', `${BASE}/_zeyn/files/${encodeURIComponent(f.store)}`, {
    'x-zeyn-csrf': token,
    'x-zeyn-file-name': encodeURIComponent(s.file.name),
    'content-type': s.file.type || 'application/octet-stream',
  }, s.file, progress);
  finish(r.status, r.text);
}

/**
 * One request of an upload, kept in `s.xhr` so Cancel can stop it. Resolves with the status
 * (`0` for a network error, `-1` when canceled), the response text and a header reader.
 */
function call(s, method, url, headers, body, onProgress) {
  return new Promise((ok) => {
    const xhr = new XMLHttpRequest();
    s.xhr = xhr;
    xhr.open(method, url);
    for (const [k, v] of Object.entries(headers)) xhr.setRequestHeader(k, v);
    if (onProgress) xhr.upload.addEventListener('progress', (e) => onProgress(e.loaded));
    xhr.addEventListener('loadend', () => {
      s.xhr = null;
      ok({ status: s.canceled ? -1 : xhr.status, text: xhr.responseText, header: (n) => xhr.getResponseHeader(n) });
    });
    xhr.send(body ?? null);
  });
}

/** Waits `ms`, or less when the upload is canceled; while the browser is offline it also waits to be online. */
function pause(s, ms) {
  return new Promise((ok) => {
    const wake = () => {
      clearTimeout(timer);
      window.removeEventListener('online', online);
      s.wake = null;
      ok();
    };
    const online = () => setTimeout(wake, 250);
    const timer = setTimeout(() => (navigator.onLine ? wake() : window.addEventListener('online', online)), ms);
    s.wake = wake;
  });
}

/**
 * The `filename` pair of `Upload-Metadata`: the UTF-8 name in base64.
 * @example meta('ü.png') // "filename w7wucG5n"
 */
function meta(name) {
  let bin = '';
  for (const b of new TextEncoder().encode(name)) bin += String.fromCharCode(b);
  return `filename ${btoa(bin)}`;
}

/**
 * Sends a file as a resumable upload (feature `resumable`, tus 1.0 subset): creates it once,
 * then asks the server for its offset and sends the rest in `PATCH`es of `CHUNK` bytes. A broken connection, a
 * busy or conflicting upload and server errors are retried after `BACKOFF` delays, waiting for
 * the browser to come back online, until `RETRIES` attempts in a row fail; a refusal (`403`,
 * `413`, `415`, `422`) ends at once. An upload the server no longer knows starts over.
 */
async function resumable(f, s, token, progress) {
  const tus = { 'tus-resumable': '1.0.0', 'x-zeyn-csrf': token };
  let tries = 0;
  for (;;) {
    let r;
    if (!s.upload) {
      r = await call(s, 'POST', `${BASE}/_zeyn/uploads/${encodeURIComponent(f.store)}`, {
        ...tus,
        'upload-length': String(s.file.size),
        'upload-metadata': meta(s.file.name),
      });
      if (r.status === 201) s.upload = r.header('location');
    }
    if (s.upload) {
      r = await call(s, 'HEAD', s.upload, tus);
      if (r.status === 404) s.upload = null;
      let at = r.status === 200 ? Number(r.header('upload-offset')) || 0 : -1;
      while (at >= 0) {
        progress(at);
        const from = at;
        r = await call(s, 'PATCH', s.upload, { ...tus, 'upload-offset': String(from), 'content-type': 'application/offset+octet-stream' }, s.file.slice(from, from + CHUNK), (n) => progress(from + n));
        if (r.status === 200) {
          s.upload = null;
          return r;
        }
        at = r.status === 204 ? Number(r.header('upload-offset')) : -1;
        if (at >= 0) tries = 0;
      }
    }
    if (r.status === -1) {
      if (s.upload) fetch(s.upload, { method: 'DELETE', headers: tus }).catch(() => {});
      s.upload = null;
      return r;
    }
    if ([403, 413, 415, 422].includes(r.status)) {
      s.upload = null;
      return r;
    }
    if (++tries > RETRIES) return r;
    await pause(s, BACKOFF[Math.min(tries, BACKOFF.length) - 1]);
    if (s.canceled) return { status: -1, text: '' };
  }
}

/** Cancels the upload of an item: the running request and any wait before a retry. */
function stop(s) {
  if (!s) return;
  s.canceled = true;
  s.xhr?.abort();
  s.wake?.();
}

/** Runs the upload action of a `FileDrop` for a stored file; a refusal marks the item failed. */
async function report(f, li, file) {
  const drop = f.root.closest('[data-zeyn-file-drop][data-save]');
  if (!drop) return;
  const screen = drop.closest('[data-zeyn-screen]');
  const r = await post('/_zeyn/data', {
    screen: screen?.dataset.zeynScreen ?? null,
    params: parse(screen?.dataset.zeynParams ?? 'null'),
    node: drop.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
    key: 'upload',
    input: file,
  });
  if (r.ok) return;
  li.removeAttribute('data-id');
  sync(f);
  failed(f, li, r.body?.message || f.texts.not_added, false);
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
  if (!allowed(f, file)) {
    failed(f, li, f.texts.type, false);
    return;
  }
  if (f.maxBytes && file.size > f.maxBytes) {
    failed(f, li, f.texts.too_large, false);
    return;
  }
  queue(f, li);
}

/** Removes an item: stops its upload, frees its preview and moves focus to a neighbor. */
function drop(f, li) {
  const s = items.get(li);
  stop(s);
  if (s?.url) URL.revokeObjectURL(s.url);
  const name = s?.file.name ?? parse(li.dataset.file ?? 'null')?.name ?? '';
  const next = li.nextElementSibling ?? li.previousElementSibling;
  const wasDone = li.hasAttribute('data-id');
  li.remove();
  if (wasDone) sync(f);
  pump(f);
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

/**
 * The files of a drop: those of `dataTransfer.files`, or, when folders were dropped, the files
 * of the whole tree that `f` accepts, read until `cap` are found. `readEntries` gives at most 100
 * entries a call, so each folder is read until it answers with none. The entries are taken before
 * the first await, while the drop data can still be read.
 */
async function dropped(f, dt, cap) {
  const entries = [...(dt?.items ?? [])]
    .filter((i) => i.kind === 'file')
    .map((i) => i.webkitGetAsEntry?.())
    .filter(Boolean);
  if (!entries.some((e) => e.isDirectory)) return [...(dt?.files ?? [])];
  const out = [];
  let seen = 0;
  let skipped = 0;
  const visit = async (entry) => {
    if (out.length >= cap || ++seen > MAX_ENTRIES || entry.name.startsWith('.')) return;
    if (entry.isFile) {
      const file = await new Promise((ok) => entry.file(ok, () => ok(null)));
      if (file && allowed(f, file)) out.push(file);
      else skipped += 1;
      return;
    }
    const reader = entry.createReader();
    for (;;) {
      const batch = await new Promise((ok) => reader.readEntries(ok, () => ok([])));
      if (!batch.length || out.length >= cap || seen > MAX_ENTRIES) return;
      for (const child of batch) await visit(child);
    }
  };
  for (const entry of entries) await visit(entry);
  if (skipped) announce(f, fill(f.texts.skipped, { n: skipped }));
  return out;
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
  if (b.classList.contains('z-upload-cancel')) stop(items.get(li));
  else if (b.classList.contains('z-upload-retry')) queue(f, li);
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
  const f = field(zone.closest('.z-upload'));
  dropped(f, e.dataTransfer, f.many ? f.maxFiles + 1 : 1).then((files) => take(f, files));
});

document.addEventListener('paste', (e) => {
  const root = e.target.closest?.('.z-upload');
  const files = e.clipboardData?.files;
  if (!root || !files?.length) return;
  e.preventDefault();
  take(field(root), files);
});

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));

new MutationObserver((records) => {
  for (const r of records) for (const n of r.addedNodes) if (n instanceof HTMLDialogElement) init(n);
}).observe(document.body, { childList: true });

init(document);
