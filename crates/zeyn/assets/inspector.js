/**
 * Property inspector of an island (`PropertyInspector::island`). The island sends `zeyn:inspect`
 * with `detail: { title, schema, values }`; the inspector checks the schema against its limits,
 * draws one field per spec by cloning the field templates the server rendered (the same markup
 * as model forms) and filling names, labels and rules in with DOM calls, never as HTML. On each
 * committed change (leaving a field, Enter, closing a picker, a step) it checks the value as
 * `FieldSpec::check` does on the server and sends `zeyn:property` with `detail: { name, value }`
 * to the island root; a value that fails shows its error and is not sent. A schema seen before
 * keeps its fields: only the values that changed are set. `detail.key` names the object (its id
 * in the island) and comes back in every `zeyn:property` (`{ key, name, value }`). While the
 * same object is shown, a field being edited (focused or changed since its last commit) keeps
 * what the user typed; when another object arrives (another key, or without keys another title
 * or schema), a pending valid edit is first sent with the old key and an invalid one is dropped,
 * so an edit never reaches an object it was not typed for. Nothing goes to the server. Ids
 * come from the field's place in the schema (`zi-0`, `zi-1`), so names never collide with the
 * `-error` and `-help` ids. Numbers are JavaScript numbers: integers beyond 2^53 lose precision.
 * The island sends `zeyn:inspect` after this module has loaded (on `DOMContentLoaded` or later);
 * an event sent before is not seen. Loaded only by pages
 * that hold an island inspector (`data-zeyn-inspector`).
 * @module inspector
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { fill, initializers, make, parse, texts } = K;

/** Field templates the server renders with this name in place of the field's. */
const TOKEN = 'zeyn-field-name';
/** Schemas whose fields are kept for reuse, per inspector. */
const CACHE = 8;
const KINDS = new Set(['text', 'multiline', 'integer', 'decimal', 'length', 'color', 'boolean', 'choice', 'expression']);
const TEXTS = new Set(['text', 'multiline', 'expression']);
const NAME = /^[A-Za-z0-9_.-]+$/;
const HEX = /^#(?:[0-9a-f]{6}|[0-9a-f]{8})$/i;

/** The limits of an inspector, from `data-limits`. */
function limits(root) {
  root.zeynLimits ??= parse(root.dataset.limits ?? '{}') ?? {};
  return root.zeynLimits;
}

/** A string cut to `max` characters; anything else is empty. */
function cut(v, max) {
  if (typeof v !== 'string') return '';
  return v.length <= max ? v : [...v].slice(0, max).join('');
}

/** A finite number, or `undefined`. */
function finite(v) {
  return typeof v === 'number' && Number.isFinite(v) ? v : undefined;
}

/** One field spec of a schema, cleaned; `null` when it cannot be drawn. */
function cleanField(f, lim) {
  if (!f || typeof f !== 'object' || !KINDS.has(f.kind)) return null;
  if (typeof f.name !== 'string' || f.name.length > lim.name || !NAME.test(f.name)) return null;
  const maxLen = Number.isInteger(f.max_len) && f.max_len >= 0 ? Math.min(f.max_len, lim.text) : lim.text;
  const options = Array.isArray(f.options) ? f.options.slice(0, lim.options) : [];
  return {
    name: f.name,
    kind: f.kind,
    label: cut(f.label, lim.text) || f.name,
    help: cut(f.help, lim.text),
    required: f.required === true,
    min: finite(f.min),
    max: finite(f.max),
    maxLen,
    scale: Number.isInteger(f.scale) && f.scale >= 0 && f.scale <= 10 ? f.scale : 4,
    fallback: f.default,
    options: options.filter((o) => o && typeof o.value === 'string' && [...o.value].length <= lim.text).map((o) => ({ value: o.value, label: cut(o.label, lim.text) || o.value })),
    units: Array.isArray(f.units) ? f.units.filter((u) => typeof u === 'string').slice(0, 5) : [],
    palette: Array.isArray(f.palette) ? f.palette.filter((c) => typeof c === 'string' && HEX.test(c)).slice(0, lim.palette) : [],
  };
}

/** The sections of a schema, cleaned; `null` when it is missing or larger than the limits. */
function clean(schema, lim) {
  const sections = schema?.sections;
  if (!Array.isArray(sections) || sections.length > lim.fields) return null;
  let count = 0;
  const seen = new Set();
  const out = [];
  for (const s of sections) {
    const fields = Array.isArray(s?.fields) ? s.fields : [];
    count += fields.length;
    if (count > lim.fields) return null;
    const specs = [];
    for (const f of fields) {
      const spec = cleanField(f, lim);
      if (spec && !seen.has(spec.name)) {
        seen.add(spec.name);
        specs.push(spec);
      }
    }
    out.push({ title: cut(s?.title, lim.text), fields: specs });
  }
  return out;
}

/** The element of a field that holds its value and its name. */
function controlOf(field) {
  return field.querySelector('[name]');
}

/** One field from its kind's template, with the spec's name, label and rules. */
function fieldNode(root, f, index) {
  const tpl = root.querySelector(`template[data-kind="${f.kind}"]`);
  const field = tpl.content.firstElementChild.cloneNode(true);
  const local = `zi-${index}`;
  for (const el of [field, ...field.querySelectorAll('*')]) {
    for (const a of [...el.attributes]) {
      if (!a.value.includes(TOKEN)) continue;
      el.setAttribute(a.name, a.value.replaceAll(TOKEN, a.name === 'name' || a.name === 'data-testid' ? f.name : local));
    }
  }
  const control = controlOf(field);
  const label = field.querySelector('label[for]');
  label.prepend(document.createTextNode(f.label));
  if (f.required) {
    label.append(make('span', 'z-required', { 'aria-hidden': 'true', text: '*' }));
    control.required = true;
    control.setAttribute('aria-required', 'true');
    field.querySelector('option[value=""]')?.remove();
  }
  const help = field.querySelector('.z-field-help');
  if (f.help) help.textContent = f.help;
  else {
    help.remove();
    control.setAttribute('aria-describedby', control.getAttribute('aria-describedby').replace(`${control.id}-help `, ''));
  }
  if (TEXTS.has(f.kind)) control.maxLength = f.maxLen;
  const wrap = field.querySelector('[data-zeyn-number], .z-length, .z-color');
  if (wrap && (f.kind === 'integer' || f.kind === 'decimal' || f.kind === 'length')) {
    for (const k of ['min', 'max']) {
      if (f[k] === undefined) continue;
      wrap.dataset[k] = String(f[k]);
      if (f.kind !== 'length') control.setAttribute(`aria-value${k}`, String(f[k]));
    }
  }
  if (f.kind === 'length' && f.units.length) wrap.dataset.units = JSON.stringify(f.units);
  if (f.kind === 'decimal' || f.kind === 'length') wrap.dataset.scale = String(f.scale);
  if (f.kind === 'color') wrap.dataset.palette = JSON.stringify(f.palette);
  if (f.kind === 'choice') for (const o of f.options) control.append(make('option', '', { value: o.value, text: o.label }));
  field.zeynSpec = f;
  field.zeynId = control.id;
  return field;
}

/** The fields of a schema, in its sections. */
function build(root, sections) {
  const box = make('div', 'z-inspector-fields');
  let index = 0;
  for (const s of sections) {
    let into = box;
    if (s.title) {
      const details = make('details', 'z-form-section', { open: '' });
      into = make('div', 'z-form-section-body', { role: 'group', 'aria-label': s.title });
      details.append(make('summary', 'z-form-section-title', { text: s.title }), into);
      box.append(details);
    }
    for (const f of s.fields) into.append(fieldNode(root, f, index++));
  }
  return box;
}

/** A number typed in a number field: group separators and spaces dropped; `null` when unreadable. */
function readNumber(text, decimal, group) {
  let plain = text.replace(/\s/g, '');
  if (group) plain = plain.split(group).join('');
  plain = plain.split(decimal || '.').join('.');
  if (!/^[-+]?(?:\d+\.?\d*|\.\d+)$/.test(plain)) return null;
  const n = Number(plain);
  return Number.isFinite(n) ? n : null;
}

/** Puts a value into a field as its control shows it. */
function setValue(field, v) {
  const f = field.zeynSpec;
  const control = controlOf(field);
  const wrap = control.closest('[data-zeyn-number], .z-length, .z-color, .z-select');
  const n = finite(v);
  if (f.kind === 'boolean') control.checked = v === true;
  else if (f.kind === 'integer' || f.kind === 'decimal' || f.kind === 'length') {
    const decimal = wrap?.dataset.decimal ?? '.';
    const scale = f.kind === 'integer' ? 0 : f.scale;
    control.value = n === undefined ? '' : String(Number(n.toFixed(scale))).replace('.', decimal);
    if (f.kind !== 'length') {
      if (n === undefined) control.removeAttribute('aria-valuenow');
      else control.setAttribute('aria-valuenow', String(n));
    }
  } else control.value = cut(v, f.kind === 'color' ? 9 : f.maxLen);
  wrap?.zeynSync?.();
}

/** Checks the value of a field as `FieldSpec::check` does: `{ value }` or `{ error }`. */
function read(root, field) {
  const f = field.zeynSpec;
  const control = controlOf(field);
  const t = texts(root);
  const error = (key, n) => ({ error: fill(t[key], { label: f.label, n }) });
  if (f.kind === 'boolean') return { value: control.checked };
  const raw = control.value;
  if (raw === '') return f.required ? error('required') : { value: null };
  if (TEXTS.has(f.kind)) return [...raw].length > f.maxLen ? error('max_len', f.maxLen) : { value: raw };
  if (f.kind === 'color') return HEX.test(raw.trim()) ? { value: raw.trim() } : error('color');
  if (f.kind === 'choice') return f.options.some((o) => o.value === raw) ? { value: raw } : error('format');
  const wrap = control.closest('[data-zeyn-number], .z-length');
  const n = readNumber(raw, wrap?.dataset.decimal, f.kind === 'length' ? '' : wrap?.dataset.group);
  if (n === null || (f.kind === 'integer' && !Number.isSafeInteger(n))) return error('format');
  if (f.kind !== 'integer' && Number(n.toFixed(f.scale)) !== n) return error('scale', f.scale);
  if (f.min !== undefined && n < f.min) return error('min', f.min);
  if (f.max !== undefined && n > f.max) return error('max', f.max);
  return { value: n };
}

/** Shows or clears the error of a field. */
function mark(field, message) {
  const p = document.getElementById(`${field.zeynId}-error`);
  if (p && p.textContent !== message) p.textContent = message;
  for (const el of [controlOf(field), document.getElementById(field.zeynId)]) {
    if (!el) continue;
    if (message) el.setAttribute('aria-invalid', 'true');
    else el.removeAttribute('aria-invalid');
  }
}

/** Checks a field and sends its value to the island when it is valid and changed. */
function commit(field) {
  const root = field.closest('[data-zeyn-inspector]');
  if (!root || !field.zeynSpec) return;
  const r = read(root, field);
  if (r.error) {
    mark(field, r.error);
    return;
  }
  mark(field, '');
  const sent = JSON.stringify(r.value);
  field.zeynDirty = false;
  field.zeynShown = sent;
  if (sent === field.zeynSent) return;
  field.zeynSent = sent;
  const island = root.zeynIsland;
  if (island?.isConnected) island.dispatchEvent(new CustomEvent('zeyn:property', { bubbles: true, detail: { key: root.zeynObjectKey ?? null, name: field.zeynSpec.name, value: r.value } }));
}

/** The object key of a `zeyn:inspect`: a string or a finite number, or `null`. */
function objectKey(detail, lim) {
  const k = detail?.key;
  if (typeof k === 'string') return cut(k, lim.text);
  return typeof k === 'number' && Number.isFinite(k) ? k : null;
}

/**
 * Before another object is shown: commits each pending edit that is valid to the object it was
 * typed for (its key and island), and drops the ones that are not.
 */
function flush(root) {
  const box = root.querySelector('.z-inspector-body')?.firstElementChild;
  if (!box) return;
  for (const field of box.querySelectorAll('.z-field')) {
    if (field.zeynDirty) commit(field);
    if (field.zeynDirty) mark(field, '');
    field.zeynDirty = false;
    field.zeynShown = undefined;
  }
}

/** Draws what an island sent: the fields of a new schema, or new values in the fields shown. */
function inspect(root, island, detail) {
  const lim = limits(root);
  const sections = clean(detail?.schema, lim);
  if (!sections) {
    console.warn(`zeyn:inspect: the schema is missing or has more than ${lim.fields} fields`);
    return;
  }
  const key = JSON.stringify(sections);
  const objectKeyNow = objectKey(detail, lim);
  const object = objectKeyNow !== null ? `k${JSON.stringify(objectKeyNow)}` : `t${cut(detail.title, lim.text)}
${key}`;
  const same = root.zeynObject === object;
  if (!same) flush(root);
  root.zeynObject = object;
  root.zeynObjectKey = objectKeyNow;
  const body = root.querySelector('.z-inspector-body');
  root.zeynIsland = island;
  let box = body.firstElementChild;
  if (root.zeynKey !== key) {
    const cache = (root.zeynCache ??= new Map());
    box = cache.get(key) ?? build(root, sections);
    cache.delete(key);
    cache.set(key, box);
    while (cache.size > CACHE) cache.delete(cache.keys().next().value);
    body.replaceChildren(box);
    if (!box.zeynReady) {
      box.zeynReady = true;
      for (const init of initializers) init(box);
    }
    root.zeynKey = key;
  }
  const values = detail.values && typeof detail.values === 'object' ? detail.values : {};
  const active = document.activeElement;
  for (const field of box.querySelectorAll('.z-field')) {
    const f = field.zeynSpec;
    if (!f || (same && (field.zeynDirty || field.contains(active)))) continue;
    const v = Object.hasOwn(values, f.name) ? values[f.name] : f.fallback;
    const shown = JSON.stringify(v ?? null);
    if (shown === field.zeynShown) continue;
    setValue(field, v);
    mark(field, '');
    const r = read(root, field);
    field.zeynShown = shown;
    field.zeynSent = r.error ? undefined : JSON.stringify(r.value);
  }
  const heading = root.querySelector('.z-inspector-title');
  root.zeynTitle ??= heading.textContent;
  const title = cut(detail.title, lim.text);
  heading.textContent = title || root.zeynTitle;
  root.querySelector('.z-inspector-empty').hidden = true;
  body.hidden = false;
  const live = root.querySelector('[data-zeyn-inspector-live]');
  const said = `${title}\n${key}`;
  if (live && title && root.zeynSaid !== said) live.textContent = fill(texts(root).shown, { title });
  root.zeynSaid = said;
}

document.addEventListener('zeyn:inspect', (e) => {
  const island = e.target instanceof Element ? e.target.closest('[data-zeyn-island]') : null;
  if (!island) return;
  for (const root of document.querySelectorAll('[data-zeyn-inspector]')) {
    if (root.dataset.zeynInspector === island.dataset.zeynIsland) inspect(root, island, e.detail);
  }
});

document.addEventListener('input', (e) => {
  const field = e.target instanceof Element ? e.target.closest('[data-zeyn-inspector] .z-field') : null;
  if (field) field.zeynDirty = true;
});

document.addEventListener('change', (e) => {
  const field = e.target instanceof Element ? e.target.closest('[data-zeyn-inspector] .z-field') : null;
  if (field) commit(field);
});

document.addEventListener('keydown', (e) => {
  if (e.key !== 'Enter' || e.isComposing) return;
  const t = e.target;
  if (!(t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement)) return;
  if (t instanceof HTMLTextAreaElement && !(e.ctrlKey || e.metaKey)) return;
  if (t instanceof HTMLInputElement && !['text', 'search', ''].includes(t.type)) return;
  const field = t.closest('[data-zeyn-inspector] .z-field');
  if (!field) return;
  e.preventDefault();
  commit(field);
});
