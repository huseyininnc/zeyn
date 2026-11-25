import { expect, test } from '@playwright/test';

/** The icon rail of the side menu keeps the app's logo mark, centered, with the name hidden. */
test('the icon rail keeps the logo mark and hides the name', async ({ page, isMobile }) => {
  test.skip(isMobile, 'the rail exists on wide screens only');
  await page.goto('/ui/card');
  await page.locator('[data-zeyn-rail-toggle]').click();
  const logo = page.locator('.z-app-name .z-logo');
  await expect(logo).toBeVisible();
  const box = await logo.boundingBox();
  const nav = await page.locator('.z-nav').boundingBox();
  expect(box && box.width).toBeGreaterThan(16);
  expect(box && nav && Math.abs(box.x + box.width / 2 - (nav.x + nav.width / 2))).toBeLessThan(2);
  const name = page.locator('.z-app-name > span:not(.z-logo)');
  expect((await name.boundingBox())?.width).toBeLessThanOrEqual(1);
  await expect(name).toHaveText('Zeyn components');
  await page.locator('[data-zeyn-rail-toggle]').click();
});
