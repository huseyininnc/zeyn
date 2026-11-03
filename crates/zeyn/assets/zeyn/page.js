/** True when the event target is a place where the user types text. */
function typing(target) {
  if (!(target instanceof Element)) return false;
  if (target.isContentEditable || target.closest('textarea, select')) return true;
  return target.matches('input') && !/^(checkbox|radio|button|submit|reset)$/.test(target.type);
}

/**
 * True when a key event matches one `aria-keyshortcuts` entry such as `Control+N`.
 * On macOS, Command (Meta) is accepted where Control is declared.
 */
function matchesShortcut(e, combo) {
  const parts = combo.split('+');
  const key = parts.pop().toLowerCase();
  const mods = new Set(parts.map((p) => p.toLowerCase()));
  const swap = MAC && mods.has('control') && !mods.has('meta');
  const ctrlMeta = swap
    ? e.ctrlKey !== e.metaKey
    : e.ctrlKey === mods.has('control') && e.metaKey === mods.has('meta');
  return sameKey(e, key) && ctrlMeta && e.shiftKey === mods.has('shift') && e.altKey === mods.has('alt');
}

/**
 * Whether the pressed key is `key` (lower case). Letters and digits also match by physical key,
 * because Option (macOS) and AltGr turn them into other characters.
 */
function sameKey(e, key) {
  if ((e.key ?? '').toLowerCase() === key) return true;
  const code = e.code ?? '';
  if (/^[a-z]$/.test(key)) return code === `Key${key.toUpperCase()}`;
  if (/^[0-9]$/.test(key)) return code === `Digit${key}`;
  return false;
}

/** Triggers the element whose `aria-keyshortcuts` matches, searching only the top dialog when one is open. */
function shortcut(e) {
  if (e.defaultPrevented || e.repeat || typing(e.target)) return;
  const scope = dialogs.at(-1)?.dialog ?? document;
  for (const el of scope.querySelectorAll('[aria-keyshortcuts]')) {
    if (el.disabled || (scope === document && el.closest('dialog'))) continue;
    if (el.getAttribute('aria-keyshortcuts').split(/\s+/).some((k) => k && matchesShortcut(e, k))) {
      e.preventDefault();
      el.click();
      return;
    }
  }
}

/**
 * Collects form values: booleans for checkboxes, strings otherwise. Skips read only inputs and
 * hidden inputs, except row versions (`data-zeyn-version`) used for conflict detection.
 */
function formValues(form) {
  const values = {};
  for (const el of form.elements) {
    if (!el.name || el.disabled || el.readOnly) continue;
    if (el.type === 'hidden' && !el.hasAttribute('data-zeyn-version')) continue;
    if (el.matches('button, fieldset, output, [type="submit"], [type="button"], [type="reset"]')) continue;
    if (el.type === 'checkbox') values[el.name] = el.checked;
    else if (el.type === 'radio') {
      if (el.checked) values[el.name] = el.value;
    } else if (el.multiple) values[el.name] = [...el.selectedOptions].map((o) => o.value).join(',');
    else values[el.name] = el.value;
  }
  return values;
}

/** Removes validation messages and `aria-invalid` from a form. */
function clearErrors(form) {
  form.querySelectorAll('.z-field-error').forEach((p) => (p.textContent = ''));
  form.querySelectorAll('[aria-invalid]').forEach((el) => el.removeAttribute('aria-invalid'));
  form.querySelector('.z-form-errors')?.remove();
}

/** Shows `422` errors under their fields, general errors on top, and focuses the first one. */
function showErrors(form, errors) {
  const scope = screenOf(form)?.dataset.zeynScreen;
  const general = [];
  let first = null;
  for (const err of errors) {
    const named = err.field ? form.elements.namedItem(err.field) : null;
    const input = named instanceof RadioNodeList ? named[0] : named;
    const id = input?.id ? `${input.id}-error` : `${scope}-${err.field}-error`;
    const box = input && form.querySelector(`#${CSS.escape(id)}`);
    if (!box) {
      general.push(err.message);
      continue;
    }
    box.textContent = box.textContent ? `${box.textContent} ${err.message}` : err.message;
    input.setAttribute('aria-invalid', 'true');
    first ??= input;
  }
  if (general.length) {
    const box = document.createElement('div');
    box.className = 'z-form-errors';
    box.setAttribute('role', 'alert');
    box.tabIndex = -1;
    for (const message of general) {
      const p = document.createElement('p');
      p.textContent = message;
      box.append(p);
    }
    form.prepend(box);
    first ??= box;
  }
  first?.focus();
  form.dispatchEvent(new CustomEvent('zeyn:errors', { bubbles: true, detail: errors }));
}

/** Validates in the browser, posts to `/_zeyn/form`, then shows errors or runs effects. */
async function submitForm(form, submitter) {
  if (form.getAttribute('aria-busy') === 'true') return;
  clearErrors(form);
  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }
  const button = submitter ?? form.querySelector('[type="submit"]');
  if (button) button.disabled = true;
  form.setAttribute('aria-busy', 'true');
  try {
    const r = await post(BASE + '/_zeyn/form', { ...address(form), values: formValues(form) });
    if (r.status === 422) showErrors(form, r.data?.errors ?? []);
    else if (r.ok) await runEffects(r.data?.effects, form);
  } finally {
    if (button) button.disabled = false;
    form.removeAttribute('aria-busy');
  }
}

/**
 * Downloads the whole filtered result from `GET /_zeyn/grid/export` as CSV or XLSX, which the
 * server builds page by page. Sends the sort, filters and column layout the user sees.
 */
function exportServer(grid, format) {
  const s = grids.get(grid);
  const a = address(grid);
  const q = JSON.stringify({ sort: s?.sort ?? [], filters: s ? G?.filterList(s) : [], columns: s?.columns ?? [] });
  const params = a.params == null ? '' : `&params=${encodeURIComponent(JSON.stringify(a.params))}`;
  const link = document.createElement('a');
  link.href = `${BASE}/_zeyn/grid/export?screen=${encodeURIComponent(a.screen)}&node=${a.node}${params}&format=${format}&q=${encodeURIComponent(q)}`;
  link.download = '';
  link.click();
}

/** Downloads the PDF document of a `PdfLink` from `GET /_zeyn/pdf`; the page stays where it is. */
function downloadPdf(button) {
  const a = address(button);
  const params = a.params == null ? '' : `&params=${encodeURIComponent(JSON.stringify(a.params))}`;
  const link = document.createElement('a');
  link.href = `${BASE}/_zeyn/pdf?screen=${encodeURIComponent(a.screen)}&node=${a.node}${params}`;
  link.download = '';
  link.click();
}

/** Quotes one CSV cell (RFC 4180) and defuses spreadsheet formulas in non numeric text. */
function csvCell(value) {
  let v = value;
  if (/^[=+\-@\t\r]/.test(v) && !/^[+-]?[\d.,\s]+$/.test(v)) v = `'${v}`;
  return /[",\r\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v;
}

/**
 * Exports the screen's first grid. Grids the server exports send every matching row in the
 * requested format; otherwise the loaded rows are saved as CSV (XLSX and PDF need the server).
 */
function exportGrid(button) {
  const first = screenOf(button)?.querySelector('[data-zeyn-grid]');
  const format = button.dataset.zeynExport;
  if ((first?.dataset.zeynExportable ?? '').split(' ').includes(format)) return exportServer(first, format);
  if (format !== 'csv') return toast(`${format.toUpperCase()} export is not available.`);
  const screen = screenOf(button);
  const grid = screen?.querySelector('[data-zeyn-grid]');
  if (!grid) return;
  const head = [...grid.querySelectorAll('thead tr:first-child th')].map((th) => th.textContent.trim());
  const s = grids.get(grid);
  const loaded =
    s?.paging === 'virtual' ? [...s.cache.keys()].sort((a, b) => a - b).flatMap((b) => s.cache.get(b)) : G?.rows(grid);
  const body = loaded.map((tr) => [...tr.cells].map((td) => td.textContent.trim()));
  const csv = [head, ...body].map((r) => r.map(csvCell).join(',')).join('\r\n') + '\r\n';
  const url = URL.createObjectURL(new Blob(['﻿', csv], { type: 'text/csv;charset=utf-8' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = `${screen.dataset.zeynScreen || 'export'}.csv`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Stores the chosen language with `POST /_zeyn/locale` and reloads the page. */
async function changeLocale(select) {
  const r = await post(BASE + '/_zeyn/locale', { locale: select.value });
  if (r.ok && !ask('zeyn:refresh-page')) location.reload();
}

/** Stores a hand picked time zone (`""` = automatic) with `POST /_zeyn/timezone` and reloads. */
async function changeTimeZone(select) {
  const r = await post(BASE + '/_zeyn/timezone', { zone: select.value });
  if (r.ok && !ask('zeyn:refresh-page')) location.reload();
}

/**
 * Applies the chosen theme at once, without a reload: the stylesheet already holds every theme,
 * so only the attributes on `<html>` and the `color-scheme` meta change. The choice is then
 * stored with `POST /_zeyn/theme`, which sets the `zeyn_theme` cookie the server renders from.
 */
async function changeTheme(select) {
  const option = select.selectedOptions[0];
  const root = document.documentElement;
  const scheme = option?.dataset.scheme ?? '';
  if (scheme) root.dataset.theme = scheme;
  else delete root.dataset.theme;
  if (option?.hasAttribute('data-custom')) root.dataset.zeynTheme = select.value;
  else delete root.dataset.zeynTheme;
  const meta = document.querySelector('meta[name="color-scheme"]');
  if (meta) meta.content = scheme || 'light dark';
  await post(BASE + '/_zeyn/theme', { theme: select.value });
}

/** Reads the page cookies into an object. */
function cookies() {
  const out = {};
  for (const part of document.cookie.split(';')) {
    const i = part.indexOf('=');
    if (i < 0) continue;
    try {
      out[part.slice(0, i).trim()] = decodeURIComponent(part.slice(i + 1).trim());
    } catch {
      continue;
    }
  }
  return out;
}

/**
 * Keeps the `zeyn_tz` cookie equal to the browser time zone. When the server rendered the page in
 * another zone (`meta[name=zeyn-tz]`) and the user did not pick one by hand (`zeyn_tz_manual`),
 * reloads once per session so the server can use the new cookie. No date math happens here.
 */
function syncTimeZone() {
  let zone;
  try {
    zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  } catch {
    return;
  }
  if (!zone) return;
  const jar = cookies();
  if (jar.zeyn_tz !== zone) {
    document.cookie = `zeyn_tz=${encodeURIComponent(zone)}; path=${BASE || '/'}; max-age=31536000; samesite=lax`;
  }
  const served = document.querySelector('meta[name="zeyn-tz"]')?.content;
  if (!served || served === zone || 'zeyn_tz_manual' in jar) return;
  try {
    if (sessionStorage.getItem('zeyn_tz_reloaded')) return;
    sessionStorage.setItem('zeyn_tz_reloaded', '1');
  } catch {
    return;
  }
  location.reload();
}

/** Abort controller of the currently registered WebMCP tools. */
let agentTools = { controller: null, names: [], handles: [], seq: 0 };

/**
 * Registers the WebMCP tools of the top visible screen and unregisters the previous ones.
 *
 * Uses `document.modelContext.registerTool(tool, { signal })` from the WebMCP draft
 * (unregistering by aborting the signal), and falls back to the older `navigator.modelContext`
 * with `unregisterTool(name)` or a returned `{ unregister() }` handle. Does nothing when the
 * browser has no model context or `GET /_zeyn/tools` answers with an error (agent tools off).
 */
async function syncAgentTools() {
  const mc = document.modelContext ?? navigator.modelContext;
  if (!mc?.registerTool) return;
  const seq = ++agentTools.seq;
  agentTools.controller?.abort();
  for (const h of agentTools.handles) h?.unregister?.();
  for (const name of agentTools.names) mc.unregisterTool?.(name);
  agentTools = { controller: null, names: [], handles: [], seq };

  const screen = dialogs.at(-1)?.dialog.querySelector('[data-zeyn-screen]') ?? document.querySelector('.z-main [data-zeyn-screen]');
  if (!screen) return;
  const name = screen.dataset.zeynScreen;
  const params = paramsOf(screen);
  let list;
  let texts;
  try {
    const q = params == null ? '' : `&params=${encodeURIComponent(JSON.stringify(params))}`;
    const res = await fetch(`${BASE}/_zeyn/tools?screen=${encodeURIComponent(name)}${q}`, { credentials: 'same-origin' });
    if (!res.ok) return;
    const body = await res.json();
    list = body?.tools ?? [];
    texts = body?.texts ?? {};
  } catch {
    return;
  }
  if (seq !== agentTools.seq) return;

  const controller = new AbortController();
  agentTools.controller = controller;
  for (const tool of list) {
    const base = { screen: name, params, node: tool.node };
    const execute = async (input) => {
      if (tool.kind === 'grid') {
        const r = await post(BASE + '/_zeyn/grid', { ...base, query: input }, { quiet: true });
        return { status: r.status, ...r.data, text: r.data?.rows ? rowsText(r.data.rows) : '' };
      }
      let confirmed = false;
      if (tool.confirm) {
        confirmed = await new Promise((resolve) => {
          if (!ask('zeyn:confirm', { ...texts, message: tool.description, danger: true, resolve })) resolve(false);
        });
        if (!confirmed) return { status: 403, error: 'confirmation_required' };
      }
      const r = await post(BASE + '/_zeyn/form', { ...base, values: input, via_agent: true, confirmed }, { quiet: true });
      if (r.ok) await runEffects(r.data?.effects);
      return { status: r.status, ...r.data };
    };
    try {
      const handle = await mc.registerTool(
        { name: tool.name, description: tool.description, inputSchema: tool.inputSchema, execute },
        { signal: controller.signal },
      );
      agentTools.names.push(tool.name);
      agentTools.handles.push(handle);
    } catch {
      continue;
    }
  }
}

/** Turns rows HTML into tab separated text lines without running any markup. */
function rowsText(html) {
  const t = document.createElement('template');
  t.innerHTML = `<table><tbody>${html}</tbody></table>`;
  return [...t.content.querySelectorAll('tr')]
    .map((tr) => [...tr.cells].map((c) => c.textContent.trim()).join('\t'))
    .join('\n');
}

