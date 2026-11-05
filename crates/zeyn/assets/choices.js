/**
 * Inline listboxes, lookups and multiple selects. Loaded only by pages that hold one
 * (`.z-listbox-field`, `data-zeyn-lookup`, `data-zeyn-multi`).
 * @module choices
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { LOOKUP_DELAY_MS, announce, clickHandlers, closePopup, fetchData, fill, fillListbox, fold, hideNative, initializers, keyHandlers, make, moveActive, openListbox, parse, pickActive, reveal, setActive, texts, typeAhead } = K;

/* Listbox control ---------------------------------------------------------------------------- */

/** Turns `[data-zeyn-listbox]` selects into always open listboxes with a search box. */
function initListboxes(root) {
  root.querySelectorAll('.z-listbox-field:not([data-ready])').forEach((wrap) => {
    const native = wrap.querySelector('select');
    if (!native) return;
    wrap.dataset.ready = '';
    const id = `${native.id}-list`;
    const list = make('div', 'z-listbox z-listbox-inline', { role: 'listbox', id, tabindex: '0', 'aria-labelledby': `${native.id}-label` });
    for (const a of ['aria-describedby', 'aria-required', 'aria-invalid']) if (native.hasAttribute(a)) list.setAttribute(a, native.getAttribute(a));
    [...native.options].forEach((o, i) => {
      const el = make('div', 'z-option', { role: 'option', id: `${id}-o${i}`, 'aria-selected': String(o.selected), 'data-value': o.value });
      el.append(make('span', 'z-option-text', { text: o.text }));
      list.append(el);
    });
    if (native.options.length > 8) {
      const search = make('input', 'z-list-search', { type: 'search', 'aria-label': wrap.dataset.textSearch, 'aria-controls': id, autocomplete: 'off', placeholder: wrap.dataset.textSearch });
      wrap.prepend(search);
    }
    wrap.append(list);
    hideNative(native);
    const selected = list.querySelector('[aria-selected="true"]') ?? list.querySelector('[role="option"]');
    if (selected) list.setAttribute('aria-activedescendant', selected.id);
  });
}

/** Chooses an option of an inline listbox (selection follows focus). */
function chooseInline(list, option) {
  if (!option) return;
  const native = list.closest('.z-listbox-field').querySelector('select');
  list.querySelectorAll('[role="option"]').forEach((o) => o.setAttribute('aria-selected', String(o === option)));
  list.setAttribute('aria-activedescendant', option.id);
  reveal(list, option);
  native.value = option.dataset.value;
  native.dispatchEvent(new Event('input', { bubbles: true }));
  native.dispatchEvent(new Event('change', { bubbles: true }));
}

/** Keyboard of an inline listbox: arrows, Home, End, Page keys and type to jump. */
function inlineKey(e, list) {
  const options = [...list.querySelectorAll('[role="option"]:not([hidden])')];
  const current = options.indexOf(document.getElementById(list.getAttribute('aria-activedescendant')));
  const to = { ArrowDown: current + 1, ArrowUp: current - 1, Home: 0, End: options.length - 1, PageDown: current + 8, PageUp: current - 8 }[e.key];
  if (to !== undefined) {
    e.preventDefault();
    chooseInline(list, options[Math.max(0, Math.min(options.length - 1, to))]);
    return true;
  }
  const at = typeAhead(e, options.map((o) => ({ label: o.textContent })), Math.max(0, current));
  if (at < 0) return false;
  e.preventDefault();
  chooseInline(list, options[at]);
  return true;
}

/** Filters an inline listbox by the search text. */
function filterInline(search) {
  const list = search.closest('.z-listbox-field').querySelector('[role="listbox"]');
  const needle = fold(search.value.trim());
  list.querySelectorAll('[role="option"]').forEach((o) => (o.hidden = !!needle && !fold(o.textContent).includes(needle)));
}

/* Lookup ------------------------------------------------------------------------------------- */

/** Turns `[data-zeyn-lookup]` id inputs into searchable comboboxes. */
function initLookups(root) {
  root.querySelectorAll('[data-zeyn-lookup]:not([data-ready])').forEach((wrap) => {
    const value = wrap.querySelector('.z-lookup-value');
    if (!value) return;
    wrap.dataset.ready = '';
    const t = texts(wrap);
    const id = value.id;
    const box = make('div', 'z-lookup-box');
    const tags = make('ul', 'z-tags', { 'aria-label': t.selected });
    const combo = make('input', 'z-lookup-input', {
      type: 'text',
      role: 'combobox',
      id,
      autocomplete: 'off',
      'aria-autocomplete': 'list',
      'aria-expanded': 'false',
      'aria-describedby': value.getAttribute('aria-describedby') ?? '',
      'data-testid': `${value.dataset.testid ?? ''}.combobox`,
    });
    if (value.required) combo.setAttribute('aria-required', 'true');
    if (value.readOnly) combo.readOnly = true;
    value.removeAttribute('id');
    hideNative(value);
    const toggle = make('button', 'z-lookup-toggle', { type: 'button', tabindex: '-1', 'aria-label': t.open });
    toggle.append(make('span', 'z-select-arrow', { 'aria-hidden': 'true' }));
    box.append(tags, combo, toggle);
    wrap.append(box, make('span', 'z-sr-only', { 'aria-live': 'polite' }));
    wrap.dataset.zeynLive = '';
    wrap.zeynLabels = new Map();
    const ids = idsOf(wrap);
    if (ids.length) {
      fetchData(wrap, wrap.dataset.zeynLookup, { ids: ids.map(Number), limit: ids.length }).then((page) => {
        for (const item of page?.items ?? []) wrap.zeynLabels.set(String(item.id), item.label);
        renderLookup(wrap);
      });
    }
  });
}

/** Ids held by a lookup, as strings. */
function idsOf(wrap) {
  return wrap.querySelector('.z-lookup-value').value.split(',').map((s) => s.trim()).filter(Boolean);
}

/** Writes ids into the submitted input and shows them: tags for several, the label for one. */
function setIds(wrap, ids) {
  const value = wrap.querySelector('.z-lookup-value');
  value.value = ids.join(',');
  value.dispatchEvent(new Event('input', { bubbles: true }));
  value.dispatchEvent(new Event('change', { bubbles: true }));
  renderLookup(wrap);
}

/** Draws the chosen values of a lookup. */
function renderLookup(wrap) {
  const t = texts(wrap);
  const combo = wrap.querySelector('[role="combobox"]');
  const ids = idsOf(wrap);
  const label = (id) => wrap.zeynLabels.get(id) ?? `#${id}`;
  if (wrap.dataset.multiple === 'true') {
    const tags = wrap.querySelector('.z-tags');
    tags.replaceChildren(
      ...ids.map((id) => {
        const li = make('li', 'z-tag');
        li.append(make('span', '', { text: label(id) }));
        const remove = make('button', 'z-tag-remove', { type: 'button', 'data-remove': id, 'aria-label': fill(t.remove, { label: label(id) }) });
        remove.append(make('span', '', { 'aria-hidden': 'true', text: '×' }));
        li.append(remove);
        return li;
      }),
    );
  } else {
    combo.value = ids.length ? label(ids[0]) : '';
    combo.dataset.shown = combo.value;
  }
}

const lookupTimers = new WeakMap();

/** Searches the server for what was typed and shows a page of options. */
async function searchLookup(wrap, offset = 0, keep = []) {
  const combo = wrap.querySelector('[role="combobox"]');
  const t = texts(wrap);
  const multiple = wrap.dataset.multiple === 'true';
  const text = multiple || combo.value !== combo.dataset.shown ? combo.value.trim() : '';
  if (K.lb?.combo !== combo) {
    openListbox(combo, {
      label: document.getElementById(`${combo.id}-label`)?.textContent.replace('*', '').trim() ?? '',
      multiple,
      anchor: wrap.querySelector('.z-lookup-box'),
      pick: (item) => pickLookup(wrap, item),
      more: () => searchLookup(wrap, K.lb.next, K.lb.items.filter((i) => !i.more)),
    });
    lookupColumns(wrap);
    fillListbox([...keep, { note: t.loading }]);
  }
  wrap.zeynAbort?.abort();
  wrap.zeynAbort = new AbortController();
  const page = await fetchData(wrap, wrap.dataset.zeynLookup, { text, offset, limit: 20 }, wrap.zeynAbort.signal);
  if (!page || K.lb?.combo !== combo) return;
  const chosen = new Set(idsOf(wrap));
  const items = [...keep];
  for (const item of page.items ?? []) {
    wrap.zeynLabels.set(String(item.id), item.label);
    items.push({ value: String(item.id), label: item.label, hint: item.hint, cells: item.cells, selected: chosen.has(String(item.id)) });
  }
  K.lb.next = page.next ? Number(page.next) : null;
  if (page.next) items.push({ value: '', label: t.more, more: true });
  if (!items.length) items.push({ note: t.no_results });
  const active = keep.length ? items[keep.length]?.value : undefined;
  fillListbox(items, active);
  announce(wrap, page.total != null ? fill(t.results, { count: page.total }) : '');
}

/** Picks an option of a lookup: replaces the value, or toggles it when several are allowed. */
function pickLookup(wrap, item) {
  const combo = wrap.querySelector('[role="combobox"]');
  if (wrap.dataset.multiple === 'true') {
    const ids = idsOf(wrap);
    const next = ids.includes(item.value) ? ids.filter((i) => i !== item.value) : [...ids, item.value];
    setIds(wrap, next);
    item.selected = !item.selected;
    item.el.setAttribute('aria-selected', String(item.selected));
    combo.focus();
    return;
  }
  setIds(wrap, [item.value]);
  closePopup(false);
  combo.focus();
}

/** Keyboard of a lookup combobox (ARIA combobox with list autocomplete). */
function lookupKey(e, combo) {
  const wrap = combo.closest('[data-zeyn-lookup]');
  const open = K.lb?.combo === combo;
  if (!open && (e.key === 'ArrowDown' || (e.altKey && e.key === 'ArrowDown'))) {
    e.preventDefault();
    searchLookup(wrap);
    return true;
  }
  if (!open) {
    if (e.key === 'Backspace' && wrap.dataset.multiple === 'true' && !combo.value) {
      const ids = idsOf(wrap);
      if (ids.length) {
        e.preventDefault();
        setIds(wrap, ids.slice(0, -1));
        return true;
      }
    }
    return false;
  }
  if (e.key === 'Enter') {
    e.preventDefault();
    return pickActive();
  }
  if (e.key === 'ArrowUp' && e.altKey) {
    e.preventDefault();
    closePopup(false);
    return true;
  }
  if (e.key === 'Home' || e.key === 'End') return false;
  return moveActive(e);
}

/** Restores a single lookup's label when the user leaves it without choosing; empty clears it. */
function leaveLookup(combo) {
  const wrap = combo.closest('[data-zeyn-lookup]');
  if (!wrap || wrap.dataset.multiple === 'true') {
    if (wrap) combo.value = '';
    return;
  }
  if (!combo.value.trim() && idsOf(wrap).length) setIds(wrap, []);
  else combo.value = combo.dataset.shown ?? '';
}

/* Multi select ------------------------------------------------------------------------------- */

/** Turns `[data-zeyn-multi]` multiple selects into chips and a combobox with a multi select list. */
function initMultis(root) {
  root.querySelectorAll('[data-zeyn-multi]:not([data-ready])').forEach((wrap) => {
    const native = wrap.querySelector('select');
    if (!native) return;
    wrap.dataset.ready = '';
    const t = texts(wrap);
    const box = make('div', 'z-lookup-box z-multi-box');
    const tags = make('ul', 'z-tags', { 'aria-label': t.selected });
    const combo = make('div', 'z-multi-button', {
      role: 'combobox',
      tabindex: native.disabled ? '-1' : '0',
      'aria-haspopup': 'listbox',
      'aria-expanded': 'false',
      'aria-labelledby': `${native.id}-label ${native.id}-value`,
      'data-testid': `${native.dataset.testid ?? ''}.combobox`,
    });
    for (const a of ['aria-describedby', 'aria-required', 'aria-invalid']) if (native.hasAttribute(a)) combo.setAttribute(a, native.getAttribute(a));
    if (native.required) combo.setAttribute('aria-required', 'true');
    combo.append(make('span', 'z-select-value', { id: `${native.id}-value` }));
    box.append(tags, combo, make('span', 'z-select-arrow', { 'aria-hidden': 'true' }));
    wrap.prepend(box);
    wrap.append(make('span', 'z-sr-only', { 'aria-live': 'polite' }));
    wrap.dataset.zeynLive = '';
    hideNative(native);
    syncMulti(wrap);
  });
}

/** Draws the chosen options of a multi select as chips and a count. */
function syncMulti(wrap) {
  const t = texts(wrap);
  const native = wrap.querySelector('select');
  const chosen = [...native.selectedOptions];
  wrap.querySelector('.z-tags').replaceChildren(
    ...chosen.map((o) => {
      const li = make('li', 'z-tag');
      li.append(make('span', '', { text: o.text }));
      const remove = make('button', 'z-tag-remove', { type: 'button', 'data-multi-remove': o.value, 'aria-label': fill(t.remove, { label: o.text }) });
      remove.append(make('span', '', { 'aria-hidden': 'true', text: '×' }));
      li.append(remove);
      return li;
    }),
  );
  const value = wrap.querySelector('.z-select-value');
  value.textContent = chosen.length ? fill(t.count, { n: chosen.length }) : t.none;
  value.classList.toggle('z-placeholder', !chosen.length);
}

/** Selects or clears one option of a multi select and tells the form. */
function setMulti(wrap, value, on) {
  const native = wrap.querySelector('select');
  const option = [...native.options].find((o) => o.value === value);
  if (!option || option.selected === on) return;
  option.selected = on;
  native.dispatchEvent(new Event('input', { bubbles: true }));
  native.dispatchEvent(new Event('change', { bubbles: true }));
  syncMulti(wrap);
  const item = K.lb?.items.find((i) => i.value === value);
  if (item) {
    item.selected = on;
    item.el.setAttribute('aria-selected', String(on));
  }
  announce(wrap, fill(texts(wrap).count, { n: native.selectedOptions.length }));
}

/** Opens the list of a multi select; options stay selected as the user toggles them. */
function openMulti(combo, where = 'first') {
  const wrap = combo.closest('[data-zeyn-multi]');
  const native = wrap.querySelector('select');
  openListbox(combo, {
    label: document.getElementById(`${native.id}-label`)?.textContent.replace('*', '').trim() ?? '',
    multiple: true,
    anchor: wrap.querySelector('.z-multi-box'),
    pick: (item) => setMulti(wrap, item.value, !item.selected),
  });
  const items = [...native.options].map((o) => ({ value: o.value, label: o.text, selected: o.selected }));
  fillListbox(items, native.selectedOptions[0]?.value);
  if (where === 'last') setActive(items.length - 1);
  if (where === 'first' && !native.selectedOptions.length) setActive(0);
}

/**
 * Keyboard of a multi select (APG listbox with multiple selection, inside a combobox): arrows,
 * Home and End move, Space and Enter toggle, Shift with an arrow moves and toggles, letters jump,
 * Backspace on the closed combobox removes the last chip. Ctrl+A is the browser's and stays so.
 */
function multiKey(e, combo) {
  const wrap = combo.closest('[data-zeyn-multi]');
  const open = K.lb?.combo === combo;
  if (!open) {
    if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
      e.preventDefault();
      openMulti(combo);
      return true;
    }
    if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      openMulti(combo, e.key === 'Home' ? 'first' : 'last');
      return true;
    }
    if (e.key === 'Backspace' || e.key === 'Delete') {
      const last = [...wrap.querySelector('select').selectedOptions].at(-1);
      if (!last) return false;
      e.preventDefault();
      setMulti(wrap, last.value, false);
      return true;
    }
    const items = [...wrap.querySelector('select').options].map((o) => ({ label: o.text }));
    const at = typeAhead(e, items, 0);
    if (at < 0) return false;
    e.preventDefault();
    openMulti(combo);
    setActive(at);
    return true;
  }
  if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault();
    return pickActive();
  }
  if (e.altKey && e.key === 'ArrowUp') {
    e.preventDefault();
    closePopup(true);
    return true;
  }
  if (e.key === 'Tab') {
    closePopup(false);
    return false;
  }
  if (e.shiftKey && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
    moveActive(e);
    return pickActive();
  }
  if (moveActive(e)) return true;
  const at = typeAhead(e, K.lb.items, Math.max(0, K.lb.active));
  if (at < 0) return false;
  e.preventDefault();
  setActive(at);
  return true;
}

/* Lookup columns ----------------------------------------------------------------------------- */

/** Gives the open lookup list the column headers of a multi column lookup (`data-columns`). */
function lookupColumns(wrap) {
  const columns = parse(wrap.dataset.columns ?? 'null');
  if (!K.lb || !Array.isArray(columns) || !columns.length) return;
  K.lb.columns = columns;
  K.lb.panel.classList.add('z-list-columns');
  K.lb.panel.style.setProperty('--z-cols', String(columns.length));
  const head = make('div', 'z-list-head', { 'aria-hidden': 'true' });
  columns.forEach((c) => head.append(make('span', '', { text: c })));
  K.lb.panel.prepend(head);
}

/* Setup of listboxes and lookups ------------------------------------------------------------- */

initializers.push(initListboxes, initLookups);

keyHandlers.push(
  (e, t) => t.matches('[data-zeyn-lookup] [role="combobox"]') && lookupKey(e, t),
  (e, t) => t.matches('.z-listbox-inline') && inlineKey(e, t),
  (e, t) => {
    if (!t.matches('.z-list-search') || !['ArrowDown', 'ArrowUp'].includes(e.key)) return false;
    const list = t.closest('.z-listbox-field').querySelector('[role="listbox"]');
    list.focus();
    return inlineKey(e, list);
  },
);

clickHandlers.push(
  (e, t) => {
    const b = t.closest('.z-lookup-toggle');
    if (!b) return false;
    const wrap = b.closest('[data-zeyn-lookup]');
    const combo = wrap.querySelector('[role="combobox"]');
    if (K.lb?.combo === combo) closePopup(true);
    else {
      combo.focus();
      searchLookup(wrap);
    }
    return true;
  },
  (e, t) => {
    const b = t.closest('[data-zeyn-lookup] [data-remove]');
    if (!b) return false;
    const wrap = b.closest('[data-zeyn-lookup]');
    setIds(wrap, idsOf(wrap).filter((id) => id !== b.dataset.remove));
    wrap.querySelector('[role="combobox"]').focus();
    return true;
  },
  (e, t) => {
    const option = t.closest('.z-listbox-inline [role="option"]');
    if (!option) return false;
    const list = option.closest('[role="listbox"]');
    chooseInline(list, option);
    list.focus();
    return true;
  },
);

document.addEventListener('input', (e) => {
  const t = e.target;
  if (!(t instanceof Element)) return;
  if (t.matches('.z-list-search')) filterInline(t);
  else if (t.matches('[data-zeyn-lookup] [role="combobox"]')) {
    const wrap = t.closest('[data-zeyn-lookup]');
    clearTimeout(lookupTimers.get(wrap));
    lookupTimers.set(wrap, setTimeout(() => searchLookup(wrap), LOOKUP_DELAY_MS));
  }
});

document.addEventListener('focusout', (e) => {
  const t = e.target;
  if (!(t instanceof Element) || !t.matches('[data-zeyn-lookup] [role="combobox"]')) return;
  if (K.lb?.combo === t) closePopup(false);
  leaveLookup(t);
});

initListboxes(document);
initLookups(document);

/* Setup of the multi select ------------------------------------------------------------------ */

initializers.push(initMultis);

keyHandlers.push((e, t) => t.matches('.z-multi-button') && multiKey(e, t));

clickHandlers.push((e, t) => {
  const remove = t.closest('[data-multi-remove]');
  if (remove) {
    const wrap = remove.closest('[data-zeyn-multi]');
    setMulti(wrap, remove.dataset.multiRemove, false);
    wrap.querySelector('[role="combobox"]').focus();
    return true;
  }
  const box = t.closest('.z-multi-box');
  if (!box) return false;
  const combo = box.querySelector('[role="combobox"]');
  if (K.lb?.combo === combo) closePopup(true);
  else {
    combo.focus();
    openMulti(combo);
  }
  return true;
});

document.addEventListener('focusin', (e) => {
  const t = e.target;
  if (t instanceof Element && t.matches('[data-zeyn-multi] .z-native')) t.closest('[data-zeyn-multi]').querySelector('[role="combobox"]')?.focus();
});

/** Validation marks set on the native multiple select show on its combobox. */
new MutationObserver((records) => {
  for (const r of records) {
    const el = r.target;
    if (!(el instanceof Element) || !el.matches('[data-zeyn-multi] .z-native')) continue;
    const combo = el.closest('[data-zeyn-multi]').querySelector('[role="combobox"]');
    if (el.getAttribute('aria-invalid') === 'true') combo?.setAttribute('aria-invalid', 'true');
    else combo?.removeAttribute('aria-invalid');
  }
}).observe(document.body, { subtree: true, attributes: true, attributeFilter: ['aria-invalid'] });

initMultis(document);

