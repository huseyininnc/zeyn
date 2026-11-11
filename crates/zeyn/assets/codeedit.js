/**
 * Code editor (`zeyn::ui::CodeEditor`, `Control::Code`): a plain `<textarea>` over a highlighted
 * copy of its text (`.z-codeedit-mirror`, hidden from assistive technology), with a line number
 * gutter. The text area stays the real input, so native editing, undo, input methods and screen
 * readers work as for any text field.
 *
 * Highlighting is line based. Each line keeps its text, the tokenizer state it starts in and the
 * state it ends in; an edit compares the new lines with the old ones from both ends, highlights
 * only the lines between, and goes on past them only while a line's starting state changed (an
 * opened block comment or string). The gutter is rebuilt only when the line count changes.
 *
 * Completion (`data-complete`) and checks (`data-check`) go to `POST /_zeyn/data` after a pause
 * in typing, never per keystroke; a newer request cancels the older one and an answer for a
 * text that changed meanwhile is dropped. A completion of a long text sends only the part around
 * the cursor, so it stays under `data-max` bytes; a check of a longer text is not sent. The list
 * follows the APG combobox pattern on the text area (`aria-activedescendant`, arrows move, Enter
 * or Tab insert, Escape closes, Ctrl+Space opens). Error markers from a check, or from form
 * validation (`zeyn:errors` with the rule `code`), are clamped to the text, underlined in the
 * mirror and listed under the editor in a live region the text area is described by.
 *
 * All text goes into the page through `textContent` and text nodes, never as markup.
 * @module codeedit
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const COMPLETE_DELAY_MS = 150;
const CHECK_DELAY_MS = 500;

/** SQL keywords, the same list as `SQL` in `ui/code.rs`. */
const SQL = new Set(
  'add all alter and as asc begin between by case check commit constraint create cross default delete desc distinct drop else end exists false fetch first foreign from full group having if in index inner insert into is join key left like limit not null offset on or order outer over partition primary references returning right rollback rows select set table then true union unique update using values view when where with'.split(' '),
);

/** Words of the expression language, the same list as `EXPRESSION` in `ui/code.rs`. */
const EXPRESSION = new Set('and between false in is like not null or true'.split(' '));

/** Operator characters of the expression language, as `OPERATORS` in `ui/code.rs`. */
const OPERATORS = '+-*/%<>=!&|^';

/** Letters, digits and `_`, the same set as Rust's `char::is_alphanumeric` and `CodeQuery::word`. */
const WORD = /[\p{Alphabetic}\p{N}_]/u;
const WORD_BEFORE = /[\p{Alphabetic}\p{N}_.]*$/u;
/**
 * At most this many characters before the cursor are read as the word being completed. Matching
 * `WORD_BEFORE` against the whole text is quadratic in its length; this slice keeps it bounded.
 */
const MAX_WORD = 200;

/** Lines longer than this are shown without highlighting, so one huge line stays cheap. */
const MAX_HIGHLIGHT_LINE = 10_000;

/** Editors by their root element. */
const editors = new WeakMap();

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

/** Whether `c` is a letter, digit or `_`. */
function wordChar(c) {
  if (!c) return false;
  const n = c.charCodeAt(0);
  if (n < 128) return (n >= 48 && n <= 57) || (n >= 65 && n <= 90) || (n >= 97 && n <= 122) || n === 95;
  return WORD.test(c);
}

/** Whether `c` is an ASCII digit. */
function digit(c) {
  return c >= '0' && c <= '9';
}

/** End of an SQL string or quoted name that goes on from `k`, and whether it is still open. */
function sqlEnd(s, k, quote) {
  while (k < s.length) {
    if (s[k] === quote) {
      if (s[k + 1] !== quote) return [k + 1, false];
      k += 2;
    } else k++;
  }
  return [s.length, true];
}

/**
 * Tokens of one line as a flat list of `class, from, to`, and the state the next line starts in:
 * 0 plain, 1 in a block comment, 2 in an SQL string, 3 in an SQL quoted name.
 */
function tokenize(lang, s, state) {
  const out = [];
  const n = s.length;
  let i = 0;
  if ((lang !== 'sql' && lang !== 'expression') || n > MAX_HIGHLIGHT_LINE) return [out, 0];
  if (state === 1) {
    const e = s.indexOf('*/');
    if (e < 0) return [['com', 0, n], 1];
    out.push('com', 0, e + 2);
    i = e + 2;
  } else if (state === 2 || state === 3) {
    const [e, open] = sqlEnd(s, 0, state === 2 ? "'" : '"');
    out.push(state === 2 ? 'str' : 'key', 0, e);
    if (open) return [out, state];
    i = e;
  }
  while (i < n) {
    const c = s[i];
    if (lang === 'sql') {
      if (c === '-' && s[i + 1] === '-') {
        out.push('com', i, n);
        return [out, 0];
      }
      if (c === '/' && s[i + 1] === '*') {
        const e = s.indexOf('*/', i + 2);
        if (e < 0) {
          out.push('com', i, n);
          return [out, 1];
        }
        out.push('com', i, e + 2);
        i = e + 2;
        continue;
      }
      if (c === "'" || c === '"') {
        const [e, open] = sqlEnd(s, i + 1, c);
        out.push(c === "'" ? 'str' : 'key', i, e);
        if (open) return [out, c === "'" ? 2 : 3];
        i = e;
        continue;
      }
    } else {
      if (c === '[') {
        const close = s.indexOf(']', i + 1);
        const e = close < 0 ? n : close + 1;
        out.push('key', i, e);
        i = e;
        continue;
      }
      if (c === '"' || c === "'") {
        let k = i + 1;
        while (k < n && s[k] !== c) k += s[k] === '\\' ? 2 : 1;
        const e = Math.min(k + 1, n);
        out.push('str', i, e);
        i = e;
        continue;
      }
      if (OPERATORS.includes(c)) {
        let e = i + 1;
        while (e < n && OPERATORS.includes(s[e])) e++;
        out.push('op', i, e);
        i = e;
        continue;
      }
    }
    if (digit(c) && !wordChar(s[i - 1])) {
      let e = i + 1;
      while (e < n && (wordChar(s[e]) || (s[e] === '.' && digit(s[e + 1])))) e++;
      out.push('num', i, e);
      i = e;
      continue;
    }
    if (wordChar(c)) {
      let e = i + 1;
      while (e < n && wordChar(s[e])) e++;
      const word = s.slice(i, e).toLowerCase();
      if ((lang === 'sql' ? SQL : EXPRESSION).has(word)) out.push('kw', i, e);
      else if (lang === 'expression' && s[e] === '(') out.push('ty', i, e);
      i = e;
      continue;
    }
    i++;
  }
  return [out, 0];
}

/** Appends `text` from `a` to `b` to `el`, in a span of `cls` when it has one. */
function piece(el, text, a, b, cls) {
  if (b <= a) return;
  if (!cls) {
    el.append(text.slice(a, b));
    return;
  }
  const span = document.createElement('span');
  span.className = cls;
  span.textContent = text.slice(a, b);
  el.append(span);
}

/** The element of one mirror line: its tokens, the error spans `marks` and its line end. */
function lineElement(text, tokens, marks) {
  const el = document.createElement('span');
  const cuts = new Set([0, text.length]);
  for (let k = 0; k < tokens.length; k += 3) cuts.add(tokens[k + 1]).add(tokens[k + 2]);
  for (const [a, b] of marks) cuts.add(a).add(b);
  const points = [...cuts].sort((x, y) => x - y);
  let t = 0;
  for (let p = 0; p + 1 < points.length; p++) {
    const a = points[p];
    const b = points[p + 1];
    while (t < tokens.length && tokens[t + 2] <= a) t += 3;
    const token = t < tokens.length && tokens[t + 1] <= a ? `z-hl-${tokens[t]}` : '';
    const marked = marks.some(([x, y]) => x <= a && b <= y);
    piece(el, text, a, b, marked ? `${token} z-codeedit-err`.trim() : token);
  }
  if (marks.some(([x, y]) => x === y)) piece(el, ' ', 0, 1, 'z-codeedit-err');
  el.append('\n');
  return el;
}

/** Highlights line `i` from `state`, puts its element in place and returns its end state. */
function paint(ed, i, state) {
  const line = ed.lines[i];
  const [tokens, end] = tokenize(ed.lang, line.text, state);
  const el = lineElement(line.text, tokens, ed.marksAt.get(i) ?? []);
  if (line.el) line.el.replaceWith(el);
  line.el = el;
  line.start = state;
  line.end = end;
  return end;
}

/**
 * Rewrites the gutter when the number of lines changed. The gutter has size containment, so its
 * lines never make the editor taller; its width comes from the number of digits.
 */
function gutter(ed) {
  const n = ed.lines.length;
  if (n === ed.numbered) return;
  ed.numbered = n;
  let text = '';
  for (let i = 1; i <= n; i++) text += i === n ? `${i}` : `${i}\n`;
  ed.gutter.textContent = text;
  ed.gutter.style.setProperty('--z-codeedit-digits', String(String(n).length));
}

/** Brings the mirror in line with the text area, painting only the lines that changed. */
function update(ed) {
  const text = ed.input.value;
  if (text === ed.text) return;
  ed.text = text;
  const next = text.split('\n');
  const old = ed.lines;
  const same = Math.min(old.length, next.length);
  let a = 0;
  while (a < same && old[a].text === next[a]) a++;
  let b = 0;
  while (b < same - a && old[old.length - 1 - b].text === next[next.length - 1 - b]) b++;
  const count = old.length;
  const gone = old.splice(a, old.length - a - b, ...next.slice(a, next.length - b).map((t) => ({ text: t, el: null, start: -1, end: -1 })));
  const after = ed.lines[a + next.length - a - b]?.el ?? null;
  for (const line of gone) line.el.remove();
  let state = a > 0 ? ed.lines[a - 1].end : 0;
  const added = next.length - a - b;
  const frag = document.createDocumentFragment();
  for (let i = a; i < a + added; i++) {
    state = paint(ed, i, state);
    frag.append(ed.lines[i].el);
  }
  ed.code.insertBefore(frag, after);
  for (let i = a + added; i < ed.lines.length && ed.lines[i].start !== state; i++) state = paint(ed, i, state);
  gutter(ed);
  if (count && count !== ed.lines.length && ed.marks.check.length) mark(ed, 'check', []);
}

/** Paints every line again, after the markers changed. */
function repaint(ed, lines) {
  for (const i of lines) {
    if (i < ed.lines.length) paint(ed, i, ed.lines[i].start);
  }
}

/** UTF-8 size of `s`, counted without allocating. */
function bytes(s) {
  let n = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    n += c < 0x80 ? 1 : c < 0x800 ? 2 : c >= 0xd800 && c <= 0xdbff ? (i++, 4) : 3;
  }
  return n;
}

/** The UTF-16 offset in `s` after `n` characters (code points) from `from`: the inverse of `points`. */
function units(s, n, from = 0) {
  let i = from;
  while (n > 0 && i < s.length) {
    const c = s.charCodeAt(i);
    i += c >= 0xd800 && c <= 0xdbff && i + 1 < s.length ? 2 : 1;
    n--;
  }
  return i;
}

/** Number of characters (code points) in `s`, as the server counts the cursor. */
function points(s) {
  let n = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c < 0xdc00 || c > 0xdfff) n++;
  }
  return n;
}

/** Posts `input` to the editor's data source `key`; `null` when it failed or was cancelled. */
async function post(ed, key, input, signal) {
  const screen = ed.root.closest('[data-zeyn-screen]');
  try {
    const r = await fetch(BASE + '/_zeyn/data', {
      method: 'POST',
      credentials: 'same-origin',
      signal,
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify({
        screen: screen?.dataset.zeynScreen ?? null,
        params: parse(screen?.dataset.zeynParams ?? 'null'),
        node: ed.root.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
        key,
        input,
      }),
    });
    return r.ok ? parse(await r.text()) : null;
  } catch {
    return null;
  }
}

/** Fills a text of the locale file with values. */
function fill(text, values) {
  return text.replace(/\{(\w+)\}/g, (m, k) => (k in values ? String(values[k]) : m));
}

/* Completion ---------------------------------------------------------------------------------- */

/** Closes the suggestion list. */
function close(ed) {
  if (!ed.list || ed.list.hidden) return;
  ed.list.hidden = true;
  ed.list.replaceChildren();
  ed.items = [];
  ed.input.removeAttribute('aria-activedescendant');
  ed.input.setAttribute('aria-expanded', 'false');
}

/** Makes option `i` the active one. */
function activate(ed, i) {
  const options = ed.list.children;
  if (!options.length) return;
  ed.active = (i + options.length) % options.length;
  for (let k = 0; k < options.length; k++) options[k].setAttribute('aria-selected', String(k === ed.active));
  const option = options[ed.active];
  ed.input.setAttribute('aria-activedescendant', option.id);
  option.scrollIntoView({ block: 'nearest' });
}

/** Places the list under the cursor: numbers only, through custom properties. */
function place(ed) {
  const ta = ed.input;
  const before = ta.value.slice(0, ta.selectionStart);
  const row = before.split('\n').length - 1;
  const line = before.slice(before.lastIndexOf('\n') + 1);
  const cols = line.length + (line.split('\t').length - 1) * 3;
  const style = getComputedStyle(ta);
  const lh = parseFloat(style.lineHeight) || 20;
  if (!ed.charWidth) {
    const ctx = document.createElement('canvas').getContext('2d');
    ctx.font = style.font;
    ed.charWidth = ctx.measureText('0000000000').width / 10 || 8;
  }
  const area = ta.getBoundingClientRect();
  const frame = ed.frame.getBoundingClientRect();
  const x = area.left - frame.left + parseFloat(style.paddingLeft) + cols * ed.charWidth - ta.scrollLeft;
  const y = area.top - frame.top + parseFloat(style.paddingTop) + (row + 1) * lh - ta.scrollTop;
  const max = Math.max(0, frame.width - Math.min(ed.list.offsetWidth || 224, frame.width));
  ed.list.style.setProperty('--z-codeedit-x', `${Math.round(Math.min(Math.max(0, x), max))}px`);
  ed.list.style.setProperty('--z-codeedit-y', `${Math.round(Math.min(Math.max(0, y), frame.height))}px`);
}

/** Shows the suggestions an action returned, or closes the list when there are none. */
function show(ed, items, forced) {
  const list = Array.isArray(items) ? items.filter((c) => c && typeof c.label === 'string') : [];
  if (!list.length) {
    close(ed);
    if (forced) ed.status.textContent = ed.texts.none ?? '';
    return;
  }
  ed.items = list;
  ed.list.replaceChildren(
    ...list.map((c, i) => {
      const li = document.createElement('li');
      li.id = `${ed.list.id}-${i}`;
      li.className = 'z-codeedit-opt';
      li.setAttribute('role', 'option');
      li.setAttribute('aria-selected', 'false');
      const label = document.createElement('span');
      label.className = 'z-codeedit-label';
      label.textContent = c.label;
      li.append(label);
      if (typeof c.detail === 'string' && c.detail) {
        const detail = document.createElement('span');
        detail.className = 'z-codeedit-detail';
        detail.textContent = c.detail;
        li.append(detail);
      }
      const kind = document.createElement('span');
      kind.className = 'z-codeedit-kind';
      kind.textContent = ed.texts.kinds?.[c.kind] ?? '';
      li.append(kind);
      return li;
    }),
  );
  const opened = ed.list.hidden;
  ed.list.hidden = false;
  ed.input.setAttribute('aria-expanded', 'true');
  place(ed);
  activate(ed, 0);
  if (forced || opened || list.length !== ed.announced) ed.status.textContent = fill(ed.texts.count ?? '', { n: list.length });
  ed.announced = list.length;
}

/** Puts suggestion `i` in place of the word before the cursor. */
function accept(ed, i) {
  const item = ed.items[i];
  const ta = ed.input;
  if (!item || ta.readOnly || ta.disabled) {
    close(ed);
    return;
  }
  const at = ta.selectionStart;
  const word = ta.value.slice(Math.max(0, at - MAX_WORD), at).match(WORD_BEFORE)[0];
  const insert = typeof item.insert === 'string' ? item.insert : item.label;
  close(ed);
  ed.skip = true;
  ta.focus();
  ta.setSelectionRange(at - word.length, ta.selectionEnd);
  if (!document.execCommand('insertText', false, insert)) {
    ta.setRangeText(insert, at - word.length, ta.selectionEnd, 'end');
    ta.dispatchEvent(new Event('input', { bubbles: true }));
  }
}

/** Asks the completion action for suggestions at the cursor. */
async function complete(ed, forced) {
  const ta = ed.input;
  const text = ta.value;
  const at = ta.selectionStart;
  let from = 0;
  let to = text.length;
  const room = Math.floor(ed.max / 3);
  if (bytes(text) > ed.max) {
    from = Math.max(0, at - Math.floor((room * 3) / 4));
    to = Math.min(text.length, from + room);
    if (/[\udc00-\udfff]/.test(text[from] ?? '')) from++;
    if (/[\ud800-\udbff]/.test(text[to - 1] ?? '')) to--;
  }
  ed.completing?.abort();
  const controller = new AbortController();
  ed.completing = controller;
  const items = await post(ed, ed.input.dataset.complete, { text: text.slice(from, to), cursor: points(text.slice(from, at)) }, controller.signal);
  if (controller.signal.aborted || ta.value !== text || ta.selectionStart !== at || document.activeElement !== ta) return;
  show(ed, items, forced);
}

/** Starts a completion after a pause when the cursor follows a word, a bracket or a dot. */
function scheduleComplete(ed, now) {
  if (!ed.list) return;
  clearTimeout(ed.completeTimer);
  const ta = ed.input;
  if (ta.readOnly || ta.disabled) return;
  const c = ta.value[ta.selectionStart - 1];
  if (!now && (ta.selectionStart !== ta.selectionEnd || !(wordChar(c) || c === '[' || c === '.'))) {
    ed.completing?.abort();
    close(ed);
    return;
  }
  ed.completeTimer = setTimeout(() => complete(ed, now), now ? 0 : COMPLETE_DELAY_MS);
}

/* Error markers ------------------------------------------------------------------------------- */

/** Markers of `errors` clamped to the lines of the text, by line index. */
function clamp(ed, errors) {
  const out = [];
  for (const e of Array.isArray(errors) ? errors : []) {
    if (!e || typeof e.message !== 'string') continue;
    const row = Math.min(Math.max(1, Number(e.line) | 0), ed.lines.length) - 1;
    const text = ed.lines[row].text;
    const a = units(text, Math.max(1, Number(e.column) | 0) - 1);
    let b = units(text, Math.max(0, Number(e.length) | 0), a);
    if (b === a) b = units(text, 1, a);
    out.push({ row, a, b, message: e.message, line: row + 1, column: points(text.slice(0, a)) + 1 });
  }
  return out;
}

/** Puts the markers of `source` (`check` or `form`) in place and lists the checked ones. */
function mark(ed, source, errors) {
  ed.marks[source] = clamp(ed, errors);
  const before = new Set(ed.marksAt.keys());
  ed.marksAt = new Map();
  for (const m of [...ed.marks.check, ...ed.marks.form]) {
    if (!ed.marksAt.has(m.row)) ed.marksAt.set(m.row, []);
    ed.marksAt.get(m.row).push([m.a, m.b]);
  }
  repaint(ed, new Set([...before, ...ed.marksAt.keys()]));
  if (source !== 'check') return;
  if (ed.marks.check.length) ed.input.setAttribute('aria-invalid', 'true');
  else if (ed.invalid) ed.input.removeAttribute('aria-invalid');
  ed.invalid = ed.marks.check.length > 0;
  const key = JSON.stringify(ed.marks.check.map((m) => [m.line, m.column, m.message]));
  if (key === ed.listed) return;
  ed.listed = key;
  ed.problems.replaceChildren(
    ...ed.marks.check.map((m) => {
      const li = document.createElement('li');
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'z-codeedit-problem';
      button.textContent = fill(ed.texts.problem ?? '{message}', { line: m.line, column: m.column, message: m.message });
      button.addEventListener('click', () => goTo(ed, m));
      li.append(button);
      return li;
    }),
  );
}

/** Selects the span of a marker in the text area. */
function goTo(ed, m) {
  let at = 0;
  for (let i = 0; i < m.row; i++) at += ed.lines[i].text.length + 1;
  ed.input.focus();
  ed.input.setSelectionRange(at + m.a, at + m.b);
}

/** Asks the check action for the problems of the text. */
async function check(ed) {
  const text = ed.input.value;
  ed.checking?.abort();
  if (bytes(text) > ed.max) {
    mark(ed, 'check', []);
    return;
  }
  const controller = new AbortController();
  ed.checking = controller;
  const errors = await post(ed, ed.input.dataset.check, { text, cursor: 0 }, controller.signal);
  if (controller.signal.aborted || ed.input.value !== text) return;
  mark(ed, 'check', errors ?? []);
}

/** Starts a check after a pause in typing. */
function scheduleCheck(ed) {
  if (!ed.input.dataset.check) return;
  clearTimeout(ed.checkTimer);
  ed.checkTimer = setTimeout(() => check(ed), CHECK_DELAY_MS);
}

/* Setup ----------------------------------------------------------------------------------------- */

/** Keys of the text area: the suggestion list when it is open, Ctrl+Space to open it. */
function onKey(ed, e) {
  if (e.isComposing || e.keyCode === 229) return;
  if (ed.list && !ed.list.hidden) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      activate(ed, ed.active + (e.key === 'ArrowDown' ? 1 : -1));
      e.preventDefault();
      return;
    }
    if ((e.key === 'Enter' || e.key === 'Tab') && !e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
      accept(ed, ed.active);
      e.preventDefault();
      return;
    }
    if (e.key === 'Escape') {
      close(ed);
      e.preventDefault();
      e.stopPropagation();
      return;
    }
  }
  if (ed.list && e.key === ' ' && e.ctrlKey && !e.altKey && !e.metaKey) {
    e.preventDefault();
    scheduleComplete(ed, true);
  }
}

/** Sets up one editor: paints every line and wires the text area. */
function setup(root) {
  const input = root.querySelector('.z-codeedit-input');
  const mirror = root.querySelector('.z-codeedit-mirror');
  if (!input || !mirror) return;
  const code = document.createElement('code');
  mirror.replaceChildren(code);
  const ed = {
    root,
    input,
    code,
    frame: root.querySelector('.z-codeedit-frame'),
    mirror,
    gutter: root.querySelector('.z-codeedit-gutter'),
    list: input.dataset.complete ? root.querySelector('.z-codeedit-list') : null,
    problems: root.querySelector('.z-codeedit-problems ul'),
    status: root.querySelector('[role="status"]'),
    lang: root.dataset.zeynCode,
    max: Number(root.dataset.max) || 65536,
    texts: parse(root.dataset.texts ?? '{}') ?? {},
    text: null,
    lines: [],
    numbered: -1,
    marks: { check: [], form: [] },
    marksAt: new Map(),
    listed: '[]',
    items: [],
    active: 0,
    skip: false,
  };
  editors.set(root, ed);
  const native = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value');
  Object.defineProperty(input, 'value', {
    configurable: true,
    get: () => native.get.call(input),
    set: (v) => {
      native.set.call(input, v);
      update(ed);
    },
  });
  update(ed);
  root.classList.add('z-codeedit-ready');
  const sync = () => {
    mirror.scrollTop = input.scrollTop;
    mirror.scrollLeft = input.scrollLeft;
    ed.gutter.scrollTop = input.scrollTop;
  };
  input.addEventListener('scroll', () => {
    sync();
    if (ed.list && !ed.list.hidden) place(ed);
  });
  input.addEventListener('input', (e) => {
    update(ed);
    if (e.isComposing) return;
    if (ed.marks.form.length) mark(ed, 'form', []);
    if (ed.skip) {
      ed.skip = false;
      close(ed);
    } else scheduleComplete(ed, false);
    scheduleCheck(ed);
  });
  input.addEventListener('compositionend', () => {
    update(ed);
    scheduleComplete(ed, false);
    scheduleCheck(ed);
  });
  input.addEventListener('change', () => update(ed));
  input.addEventListener('keydown', (e) => onKey(ed, e));
  input.addEventListener('blur', () => close(ed));
  input.form?.addEventListener('reset', () => setTimeout(() => update(ed)));
  if (ed.list) {
    ed.list.addEventListener('pointerdown', (e) => e.preventDefault());
    ed.list.addEventListener('click', (e) => {
      const option = e.target.closest('[role="option"]');
      if (option) accept(ed, [...ed.list.children].indexOf(option));
    });
  }
  if (ed.list) input.setAttribute('aria-expanded', 'false');
  if (input.dataset.check && input.value) scheduleCheck(ed);
}

/** Sets up the editors under `root` that are not set up yet. */
function init(root) {
  for (const el of root.querySelectorAll('[data-zeyn-code]')) {
    if (!editors.has(el)) setup(el);
  }
}

document.addEventListener('zeyn:errors', (e) => {
  const errors = Array.isArray(e.detail) ? e.detail : [];
  for (const el of e.target.querySelectorAll?.('[data-zeyn-code]') ?? []) {
    const ed = editors.get(el);
    if (!ed) continue;
    update(ed);
    const own = errors.filter((x) => x?.field === ed.input.name && x.rule?.rule === 'code');
    mark(ed, 'form', own.map((x) => ({ ...x.rule.value, message: String(x.message ?? '') })));
  }
});
document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
