import { test, expect } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { groups, open } from './checks';

/** What one catalog page loads: script and stylesheet bytes as sent, and first contentful paint. */
type Budget = { js: number; css: number; fcp: number };

const file = join(__dirname, '..', 'budgets.json');
const budgets: Record<string, Budget> = JSON.parse(readFileSync(file, 'utf8'));
const record = !!process.env.ZEYN_E2E_RECORD_BUDGETS;
const measured: Record<string, Budget> = {};

/** First paint budgets have a floor, since one machine's timing is noisy. */
const FCP_FLOOR = 1000;

/** Room over a budget before the test fails. */
const ROOM = 1.1;

test.describe('budgets', () => {
  test.describe.configure({ mode: 'default' });
  for (const entries of Object.values(groups)) {
    for (const { slug, route } of entries) {
      test(slug, async ({ page }, info) => {
        await open(page, route, info);
        await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
        const now = await page.evaluate(() => {
          const sum = (ext: string) =>
            (performance.getEntriesByType('resource') as PerformanceResourceTiming[])
              .filter((r) => new URL(r.name).pathname.endsWith(ext))
              .reduce((n, r) => n + (r.encodedBodySize || r.transferSize), 0);
          const paint = performance.getEntriesByName('first-contentful-paint')[0];
          return { js: sum('.js'), css: sum('.css'), fcp: Math.round(paint?.startTime ?? 0) };
        });
        measured[slug] = now;
        if (record) return;
        const b = budgets[slug];
        expect(b, `${slug} has a budget in e2e/budgets.json`).toBeTruthy();
        expect(now.js, `${slug} JS bytes`).toBeLessThanOrEqual(b.js * ROOM);
        expect(now.css, `${slug} CSS bytes`).toBeLessThanOrEqual(b.css * ROOM);
        expect(now.fcp, `${slug} first paint ms`).toBeLessThanOrEqual(b.fcp * ROOM);
      });
    }
  }

  test.afterAll(() => {
    if (!record) return;
    const out: Record<string, Budget> = {};
    for (const entries of Object.values(groups)) {
      for (const { slug } of entries) {
        const m = measured[slug];
        if (!m) continue;
        out[slug] = {
          js: m.js,
          css: m.css,
          fcp: Math.max(FCP_FLOOR, Math.ceil((m.fcp * 1.5) / 100) * 100),
        };
      }
    }
    writeFileSync(file, JSON.stringify(out, null, 2) + '\n');
  });
});
