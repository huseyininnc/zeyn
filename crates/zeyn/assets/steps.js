/**
 * Forms in steps (wizards). Loaded only by pages that hold one (`data-zeyn-steps`).
 * @module steps
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { announce, clickHandlers, fetchData, fill, initializers, make } = K;

/* Steps -------------------------------------------------------------------------------------- */

/** Form values as the runtime sends them: booleans for checkboxes, strings otherwise. */
function valuesOf(form) {
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

/** Shows one step of a stepped form, updates the step list and the buttons, and announces it. */
function showStep(form, n, focus = true) {
  const steps = [...form.querySelectorAll('fieldset[data-zeyn-step]')];
  const list = form.querySelector('[data-zeyn-steps]');
  const total = steps.length;
  n = Math.max(0, Math.min(total - 1, n));
  form.dataset.step = String(n);
  steps.forEach((s, i) => (s.hidden = i !== n));
  list?.querySelectorAll('li').forEach((li, i) => {
    if (i === n) li.setAttribute('aria-current', 'step');
    else li.removeAttribute('aria-current');
    li.classList.toggle('z-step-done', i < n);
  });
  const back = form.querySelector('[data-zeyn-step-back]');
  const next = form.querySelector('[data-zeyn-step-next]');
  const submit = form.querySelector('.z-form-actions [type="submit"]');
  if (back) back.hidden = n === 0;
  if (next) next.hidden = n === total - 1;
  if (submit) submit.hidden = n !== total - 1;
  if (list) announce(form, fill(list.dataset.textProgress, { n: n + 1, total }));
  if (focus) (steps[n].querySelector('input:not([type="hidden"]):not(.z-native), select:not(.z-native), textarea, [role="combobox"], [role="listbox"]') ?? steps[n]).focus();
}

/** Sets up stepped forms: one step at a time, with the step buttons shown. */
function initSteps(root) {
  root.querySelectorAll('form:has([data-zeyn-steps]):not([data-step])').forEach((form) => {
    form.dataset.zeynLive = '';
    if (!form.querySelector(':scope > .z-sr-only[aria-live]')) form.append(make('span', 'z-sr-only', { 'aria-live': 'polite' }));
    showStep(form, 0, false);
  });
}

/** Writes errors under their fields, as the runtime does, and focuses the first one. */
function showFieldErrors(form, errors) {
  const scope = form.closest('[data-zeyn-screen]')?.dataset.zeynScreen;
  form.querySelectorAll('.z-field-error').forEach((p) => (p.textContent = ''));
  form.querySelectorAll('[aria-invalid="true"]').forEach((el) => el.removeAttribute('aria-invalid'));
  let first = null;
  for (const err of errors) {
    const named = form.elements.namedItem(err.field);
    const input = named instanceof RadioNodeList ? named[0] : named;
    const box = document.getElementById(input?.id ? `${input.id}-error` : `${scope}-${err.field}-error`) ?? document.getElementById(`${scope}-${err.field}-error`);
    if (box) box.textContent = box.textContent ? `${box.textContent} ${err.message}` : err.message;
    input?.setAttribute?.('aria-invalid', 'true');
    first ??= input;
  }
  first?.focus?.();
}

/** Next: checks the step's fields on the server, then moves on when they are fine. */
async function nextStep(button) {
  const form = button.closest('form');
  const n = Number(form.dataset.step);
  button.disabled = true;
  const data = await fetchData(form, '_step', { step: n, values: valuesOf(form) });
  button.disabled = false;
  const errors = data?.errors ?? [];
  if (errors.length) {
    showFieldErrors(form, errors);
    return;
  }
  form.querySelectorAll('.z-field-error').forEach((p) => (p.textContent = ''));
  showStep(form, n + 1);
}

/** After a failed save, shows the step that holds the first error. */
function stepOfErrors(form, errors) {
  if (!form.matches('[data-step]')) return;
  for (const err of errors) {
    const named = form.elements.namedItem(err.field);
    const input = named instanceof RadioNodeList ? named[0] : named;
    const step = input?.closest('fieldset[data-zeyn-step]');
    if (step) {
      showStep(form, Number(step.dataset.zeynStep), false);
      (form.querySelector('[aria-invalid="true"]:not(.z-native)') ?? input).focus();
      return;
    }
  }
}

/* Setup -------------------------------------------------------------------------------------- */

initializers.push(initSteps);

clickHandlers.push((e, t) => {
  const b = t.closest('[data-zeyn-step-next], [data-zeyn-step-back]');
  if (!b) return false;
  if (b.hasAttribute('data-zeyn-step-next')) nextStep(b);
  else showStep(b.closest('form'), Number(b.closest('form').dataset.step) - 1);
  return true;
});

document.addEventListener('zeyn:errors', (e) => {
  if (e.target instanceof HTMLFormElement) stepOfErrors(e.target, e.detail ?? []);
});

initSteps(document);
