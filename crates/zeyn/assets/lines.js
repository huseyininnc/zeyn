/**
 * The lines editor of forms. Loaded only by pages that hold one (`data-zeyn-lines`).
 * @module lines
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { clickHandlers, fill } = K;

/* Lines -------------------------------------------------------------------------------------- */

/** Renames the cells of every line after an add, remove or move, so names stay `{field}.{row}.{column}`. */
function renumberLines(wrap) {
  const name = wrap.dataset.zeynLines;
  const heads = [...wrap.querySelectorAll('thead th')].map((th) => th.textContent.trim());
  const base = wrap.querySelector(`input[name="${CSS.escape(name)}"]`)?.id ?? name;
  const rows = [...wrap.querySelectorAll('tbody > tr[data-zeyn-line]')];
  rows.forEach((tr, i) => {
    const n = i + 1;
    const rowText = fill(wrap.dataset.textRow, { n });
    tr.querySelector('.z-lines-n').firstChild.textContent = String(n);
    tr.querySelectorAll('input[name]').forEach((input) => {
      const col = input.name.split('.').pop();
      input.name = `${name}.${i}.${col}`;
      if (!input.dataset.col) return;
      const id = `${base}-${i}-${col}`;
      const error = input.nextElementSibling;
      input.id = id;
      input.setAttribute('aria-describedby', `${id}-error`);
      if (error) error.id = `${id}-error`;
      const cellIndex = [...tr.children].indexOf(input.closest('td'));
      input.setAttribute('aria-label', `${heads[cellIndex]}, ${rowText}`);
    });
    tr.querySelector('[data-zeyn-line-move="-1"]')?.setAttribute('aria-label', fill(wrap.dataset.textUp, { n }));
    tr.querySelector('[data-zeyn-line-move="1"]')?.setAttribute('aria-label', fill(wrap.dataset.textDown, { n }));
    tr.querySelector('[data-zeyn-line-remove]')?.setAttribute('aria-label', fill(wrap.dataset.textRemove, { n }));
  });
  wrap.querySelector('[data-zeyn-lines-empty]').hidden = rows.length > 0;
  sumLines(wrap);
}

/** Reads money typed in the line editor's locale as minor units; `null` when unreadable. */
function minorUnits(wrap, text, scale) {
  const d = wrap.dataset;
  const plain = text.split(d.group).join('').replace(/\s/g, '').replace(d.decimal, '.');
  if (!/^-?\d+(\.\d+)?$/.test(plain)) return null;
  const [whole, frac = ''] = plain.replace('-', '').split('.');
  if (frac.length > scale) return null;
  const value = BigInt(whole) * 10n ** BigInt(scale) + BigInt(frac.padEnd(scale, '0') || '0');
  return plain.startsWith('-') ? -value : value;
}

/** Writes minor units as an amount in the line editor's locale. */
function writeMinor(wrap, minor) {
  const d = wrap.dataset;
  const negative = minor < 0n;
  const abs = (negative ? -minor : minor).toString().padStart(3, '0');
  const whole = abs.slice(0, -2).replace(/\B(?=(\d{3})+(?!\d))/g, d.group);
  return `${negative ? '-' : ''}${whole}${d.decimal}${abs.slice(-2)}`;
}

/** Updates every line total and the footer total from what is typed. Display only: the server works them out again. */
function sumLines(wrap) {
  const sumCell = wrap.querySelector('[data-zeyn-lines-sum]');
  if (!sumCell) return;
  let sum = 0n;
  wrap.querySelectorAll('tbody > tr[data-zeyn-line]').forEach((tr) => {
    const qty = minorUnits(wrap, tr.querySelector('[data-zeyn-qty]')?.value ?? '', 0);
    const price = minorUnits(wrap, tr.querySelector('[data-zeyn-price]')?.value ?? '', 2);
    const cell = tr.querySelector('[data-zeyn-line-total]');
    if (qty === null || price === null) {
      if (cell) cell.textContent = '';
      return;
    }
    const total = qty * price;
    sum += total;
    if (cell) cell.textContent = writeMinor(wrap, total);
  });
  sumCell.textContent = writeMinor(wrap, sum);
}

/** Adds an empty line at the end and moves focus into it. */
function addLine(wrap) {
  const template = wrap.querySelector('template[data-zeyn-line-template]');
  const row = template.content.firstElementChild.cloneNode(true);
  wrap.querySelector('tbody').append(row);
  renumberLines(wrap);
  row.querySelector('input[data-col]:not([readonly])')?.focus();
}

/** Removes a line; focus goes to the same place in the next line, or to the add button. */
function removeLine(button) {
  const wrap = button.closest('[data-zeyn-lines]');
  const tr = button.closest('tr');
  const next = tr.nextElementSibling ?? tr.previousElementSibling;
  tr.remove();
  renumberLines(wrap);
  (next?.querySelector('[data-zeyn-line-remove]') ?? wrap.querySelector('[data-zeyn-line-add]')).focus();
}

/** Moves a line up or down, keeping focus on the button that moved it. */
function moveLine(button) {
  const wrap = button.closest('[data-zeyn-lines]');
  const tr = button.closest('tr');
  const by = Number(button.dataset.zeynLineMove);
  const other = by < 0 ? tr.previousElementSibling : tr.nextElementSibling;
  if (!other) return;
  if (by < 0) other.before(tr);
  else other.after(tr);
  renumberLines(wrap);
  button.focus();
}

/** Puts the server's line errors in their cells and keeps only other messages under the table. */
function showLineErrors(form, errors) {
  let first = null;
  for (const wrap of form.querySelectorAll('[data-zeyn-lines]')) {
    const name = wrap.dataset.zeynLines;
    const box = document.getElementById(`${wrap.querySelector(`input[name="${CSS.escape(name)}"]`)?.id}-error`);
    const own = errors.filter((e) => e.field === name);
    if (!own.length) continue;
    for (const e of own.filter((x) => x.rule?.rule === 'line')) {
      const input = wrap.querySelector(`[name="${CSS.escape(`${name}.${e.rule.value.row}.${e.rule.value.field}`)}"]`);
      if (!input) continue;
      input.setAttribute('aria-invalid', 'true');
      const cell = document.getElementById(`${input.id}-error`);
      if (cell) cell.textContent = e.message;
      first ??= input;
    }
    if (box) box.textContent = own.filter((x) => x.rule?.rule !== 'line').map((x) => x.message).join(' ');
  }
  first?.focus();
}

/* Setup -------------------------------------------------------------------------------------- */

clickHandlers.push((e, t) => {
  const b = t.closest('[data-zeyn-line-add], [data-zeyn-line-remove], [data-zeyn-line-move]');
  if (!b) return false;
  if (b.hasAttribute('data-zeyn-line-add')) addLine(b.closest('[data-zeyn-lines]'));
  else if (b.hasAttribute('data-zeyn-line-remove')) removeLine(b);
  else moveLine(b);
  return true;
});

document.addEventListener('input', (e) => {
  const t = e.target;
  if (t instanceof Element && t.matches('[data-zeyn-lines] input[data-col]')) sumLines(t.closest('[data-zeyn-lines]'));
});

document.addEventListener('zeyn:errors', (e) => {
  if (e.target instanceof HTMLFormElement) showLineErrors(e.target, e.detail ?? []);
});
