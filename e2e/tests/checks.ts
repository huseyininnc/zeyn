import { test, expect, type Page, type TestInfo } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/** One catalog page, as `e2e/components.json` lists it. */
export type Entry = { slug: string; route: string };

/** The catalog pages by category, kept in sync with the catalog by a Rust test. */
export const groups: Record<string, Entry[]> = JSON.parse(
  readFileSync(join(__dirname, '..', 'components.json'), 'utf8'),
);

/** Known findings that are not fixed yet, as `slug:check` to the reason. */
const known: Record<string, string> = JSON.parse(
  readFileSync(join(__dirname, '..', 'known-issues.json'), 'utf8'),
);

/** Whether a finding of this page and check is listed as a known issue. */
function isKnown(slug: string, check: string): boolean {
  return `${slug}:${check}` in known;
}

/** Pages whose text follows today's date or the clock, so their tree has no snapshot. */
const dated = new Set(['calendar', 'week_schedule', 'feed', 'message_thread', 'composer', 'inbox', 'timeline']);

/** Widgets with one tab stop and arrow keys inside (W3C APG), and the key that moves focus. */
const composites: [string, string][] = [
  ['[role="tablist"]', 'ArrowRight'],
  ['[role="menubar"]', 'ArrowRight'],
  ['[role="radiogroup"]', 'ArrowRight'],
  ['[role="toolbar"]', 'ArrowRight'],
  ['[role="tree"]', 'ArrowDown'],
  ['[role="listbox"]', 'ArrowDown'],
  ['[role="grid"]', 'ArrowDown'],
];

/** What counts as a pointer target for the touch check (WCAG 2.5.8). */
const targets =
  'a[href], button, input:not([type="hidden"]), select, textarea, summary, [role="button"], [role="tab"], [role="menuitem"], [role="checkbox"], [role="radio"], [role="switch"], [role="option"], [role="slider"]';

/**
 * Opens a catalog page. Requests to other hosts get an empty answer so the run needs no network,
 * console errors and page errors are collected, and the `rtl` project turns the page right to
 * left before its scripts run.
 */
export async function open(page: Page, route: string, info: TestInfo): Promise<string[]> {
  const errors: string[] = [];
  const origin = new URL(info.project.use.baseURL!).origin;
  await page.route(
    (url) => url.origin !== origin,
    (r) => r.fulfill({ status: 200, contentType: 'text/html', body: '' }),
  );
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  page.on('pageerror', (e) => errors.push(String(e)));
  if (info.project.name === 'rtl') {
    await page.addInitScript(() => {
      new MutationObserver((_, o) => {
        if (document.documentElement) {
          document.documentElement.dir = 'rtl';
          o.disconnect();
        }
      }).observe(document, { childList: true });
    });
  }
  const res = await page.goto(route);
  expect(res?.status(), `${route} answers 200`).toBe(200);
  await expect(page.locator('main .cx-demo').first()).toBeVisible();
  return errors;
}

/** Serious and critical axe findings of the page, as `rule: target` lines. */
async function axe(page: Page): Promise<string[]> {
  const result = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
    .analyze();
  return result.violations
    .filter((v) => v.impact === 'serious' || v.impact === 'critical')
    .flatMap((v) => v.nodes.map((n) => `${v.id}: ${n.target.join(' ')}`));
}

/**
 * Whether focus changes how the focused element looks. The styles of the element, its pseudo
 * elements, its first child and the two boxes around it are compared focused and blurred; when
 * they match (a ring drawn on a slider thumb, say), pictures of the area are compared instead.
 */
async function focusShown(page: Page): Promise<boolean> {
  const styled = await page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    const parts = (): string =>
      [el, el.firstElementChild, el.parentElement, el.parentElement?.parentElement]
        .flatMap((e) => (e ? [null, '::before', '::after'].map((p) => getComputedStyle(e, p)) : []))
        .map((s) => `${s.outline}|${s.boxShadow}|${s.borderColor}|${s.backgroundColor}`)
        .join(';');
    const focused = parts();
    el.blur();
    const plain = parts();
    el.focus({ preventScroll: true });
    return focused !== plain;
  });
  if (styled) return true;
  const box = await page.evaluate(() => {
    const r = document.activeElement!.getBoundingClientRect();
    return { x: Math.max(0, r.x - 6), y: Math.max(0, r.y - 6), width: r.width + 12, height: r.height + 12 };
  });
  if (box.width < 1 || box.height < 1) return false;
  const focused = await page.screenshot({ clip: box, animations: 'disabled', caret: 'hide' });
  const el = await page.evaluateHandle(() => document.activeElement as HTMLElement);
  await el.evaluate((e) => e.blur());
  const plain = await page.screenshot({ clip: box, animations: 'disabled', caret: 'hide' });
  await el.evaluate((e) => e.focus({ preventScroll: true }));
  return !focused.equals(plain);
}

/**
 * Tabs from the page heading through the component's examples. Every stop must be an element in
 * the page, not the body, and must show a focus ring.
 */
async function keyboardPath(page: Page): Promise<string[]> {
  const problems: string[] = [];
  await page.evaluate(() => {
    const h = document.querySelector('main h1') as HTMLElement;
    h.tabIndex = -1;
    h.focus();
  });
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press('Tab');
    const where = await page.evaluate(() => {
      const el = document.activeElement;
      if (!el || el === document.body) return 'body';
      if (!el.closest('main')) return 'out';
      return `${el.tagName.toLowerCase()}${el.className ? '.' + String(el.className).split(' ')[0] : ''}`;
    });
    if (where === 'out') break;
    if (where === 'body') {
      if (i === 0) problems.push('the first Tab press left focus on the body');
      break;
    }
    if (!(await focusShown(page))) problems.push(`no visible focus on ${where}`);
  }
  return problems;
}

/**
 * The first composite widget of each kind gets its tab stop focused and the arrow key pressed:
 * focus (or the active descendant) has to move and stay inside the widget.
 */
async function arrowKeys(page: Page): Promise<string[]> {
  const problems: string[] = [];
  for (const [selector, key] of composites) {
    const widget = page
      .locator(`main .cx-demo ${selector}`)
      .filter({ visible: true })
      .first();
    if ((await widget.count()) === 0) continue;
    const before = await widget.evaluate((w) => {
      const items =
        '[role="tab"], [role^="menuitem"], [role="radio"], [role="treeitem"], [role="option"], [role="gridcell"], [role="columnheader"], [role="rowheader"], [role="row"], tr, [role="button"], button, input';
      const stop =
        (w.querySelector(`:is(${items})[tabindex="0"]`) as HTMLElement | null) ??
        (w.querySelector('[tabindex="0"]') as HTMLElement | null) ??
        (w.getAttribute('tabindex') === '0' ? (w as HTMLElement) : null) ??
        (w.querySelector('input:not([disabled]), button:not([disabled])') as HTMLElement | null);
      if (!stop) return null;
      stop.focus();
      const a = document.activeElement as HTMLElement;
      return `${a.outerHTML.slice(0, 120)}|${a.getAttribute('aria-activedescendant') ?? ''}`;
    });
    if (before === null) {
      problems.push(`${selector} has no tab stop`);
      continue;
    }
    await page.keyboard.press(key);
    const after = await widget.evaluate((w) => {
      const a = document.activeElement as HTMLElement;
      if (!w.contains(a)) return null;
      return `${a.outerHTML.slice(0, 120)}|${a.getAttribute('aria-activedescendant') ?? ''}`;
    });
    if (after === null) problems.push(`${key} moved focus out of ${selector}`);
    else if (after === before) problems.push(`${key} did not move focus in ${selector}`);
  }
  return problems;
}

/**
 * Pointer targets in the examples smaller than 24 by 24 CSS pixels whose 24 pixel square around
 * the center overlaps another target (WCAG 2.5.8). Links inside a sentence are exempt.
 */
async function smallTargets(page: Page): Promise<string[]> {
  return page.evaluate((sel) => {
    const visible = (el: Element) => {
      const r = el.getBoundingClientRect();
      const s = getComputedStyle(el);
      return (
        r.width > 1 &&
        r.height > 1 &&
        s.visibility !== 'hidden' &&
        s.opacity !== '0' &&
        !el.closest('[inert], [hidden], .z-sr-only')
      );
    };
    const inSentence = (el: Element) =>
      el.tagName === 'A' &&
      getComputedStyle(el).display === 'inline' &&
      (el.parentElement?.textContent ?? '').trim().length > (el.textContent ?? '').trim().length;
    const all = [...document.querySelectorAll(`main .cx-demo :is(${sel})`)].filter(visible);
    const boxes = all.map((el) => el.getBoundingClientRect());
    const out: string[] = [];
    all.forEach((el, i) => {
      const r = boxes[i];
      if ((r.width >= 24 && r.height >= 24) || inSentence(el)) return;
      const cx = r.left + r.width / 2;
      const cy = r.top + r.height / 2;
      const clash = all.some((other, j) => {
        if (j === i || other.contains(el) || el.contains(other)) return false;
        const o = boxes[j];
        return o.left < cx + 12 && o.right > cx - 12 && o.top < cy + 12 && o.bottom > cy - 12;
      });
      if (clash) {
        const name = `${el.tagName.toLowerCase()}${el.className ? '.' + String(el.className).split(' ')[0] : ''}`;
        out.push(`${name} ${Math.round(r.width)}x${Math.round(r.height)}`);
      }
    });
    return [...new Set(out)];
  }, targets);
}

/**
 * Whether the page scrolls sideways as a whole (grids and canvases scroll in their own box).
 * When it does, the findings name the outermost elements that reach past the viewport, so a CI
 * failure says where to look.
 */
async function sidewaysOverflow(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const root = document.documentElement;
    if (root.scrollWidth <= root.clientWidth + 1) return [];
    const width = root.clientWidth;
    const out: string[] = [`page ${root.scrollWidth}px wide in ${width}px`];
    for (const el of Array.from(document.body.querySelectorAll<HTMLElement>('*'))) {
      const r = el.getBoundingClientRect();
      if (r.right <= width + 1 && r.left >= -1) continue;
      let clipped = false;
      for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
        if (getComputedStyle(a).overflowX !== 'visible') {
          clipped = true;
          break;
        }
      }
      if (clipped) continue;
      const cls = typeof el.className === 'string' && el.className ? '.' + el.className.split(' ').join('.') : '';
      out.push(`${el.tagName.toLowerCase()}${cls} ${Math.round(r.left)}..${Math.round(r.right)}`);
      if (out.length > 6) break;
    }
    return out;
  });
}

/** Fails the check with its findings unless the page and check are listed as known. */
function report(slug: string, check: string, findings: string[]) {
  if (isKnown(slug, check)) return;
  expect(findings, `${check} on ${slug}`).toEqual([]);
}

/**
 * The browser tests of one category: one test per component page, whose checks depend on the
 * project (light, dark, rtl, touch).
 */
export function componentGroup(category: string) {
  test.describe(category, () => {
    for (const { slug, route } of groups[category]) {
      test(slug, async ({ page }, info) => {
        const errors = await open(page, route, info);
        const project = info.project.name;
        if (project === 'light' || project === 'dark') {
          report(slug, 'axe', await axe(page));
        }
        if (project === 'light') {
          report(slug, 'keyboard', await keyboardPath(page));
          report(slug, 'arrows', await arrowKeys(page));
          if (!dated.has(slug)) {
            await page.reload();
            await expect(page.locator('main')).toMatchAriaSnapshot({ name: `${slug}.aria.yml` });
          }
        }
        if (project === 'rtl') {
          expect(await page.evaluate(() => document.dir)).toBe('rtl');
          report(slug, 'rtl-overflow', await sidewaysOverflow(page));
        }
        if (project === 'touch') {
          report(slug, 'targets', await smallTargets(page));
          report(slug, 'reflow', await sidewaysOverflow(page));
        }
        report(slug, 'console', errors);
      });
    }
  });
}
