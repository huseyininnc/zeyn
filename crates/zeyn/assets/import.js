/**
 * The import component: reads a CSV or XLSX file, shows the server's preview, then imports.
 * Loaded only by pages that hold one (`data-zeyn-import`).
 * @module import
 */
const B = await import(new URL(`enterprise.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { announce, post, t } = B;

const MAX_CSV_BYTES = 8 * 1024 * 1024;

/** Reads a binary file (an XLSX workbook) as base64 for `POST /_zeyn/import`. */
function readBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).slice(String(reader.result).indexOf(',') + 1));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

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
  const content = /\.xlsx$/i.test(file.name) ? { xlsx: await readBase64(file) } : { csv: await readCsv(file) };
  imports.set(box, { content, name: file.name.replace(/\.[^.]*$/, '') });
  showImport(box, await post('/_zeyn/import', { ...address(box), ...content, commit: false }));
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
  const r = await post('/_zeyn/import', { ...address(box), ...state.content, commit: true });
  showImport(box, r);
  if (r.ok) {
    box.querySelector('input[type="file"]').value = '';
    imports.delete(box);
    box.querySelector('[data-zeyn-import-report]').hidden = true;
    announce(r.data?.message ?? '');
  }
});

