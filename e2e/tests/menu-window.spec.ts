import { expect, test, type Page } from '@playwright/test';

/**
 * Side menu items that open a window (`Menu::window`): the catalog's "About this catalog" item
 * under Start. Choosing it opens a modal dialog without changing the address; Escape closes it
 * and focus goes back to the item, or to the menu button when the item sits in a closed drawer.
 */

const item = '.z-nav .z-nav-group:not(.z-nav-favorites) .z-nav-window';

/** The heading button of the top level group that holds the item. */
const groupHeading = '.z-nav-body > .z-nav-group:not(.z-nav-favorites):has(.z-nav-window) > h2 > .z-nav-heading';

/** Shows the item: opens the drawer on narrow screens and the group when it is closed. */
async function reveal(page: Page) {
  const toggle = page.locator('[data-zeyn-nav-toggle]');
  if (await toggle.isVisible()) await toggle.click();
  const heading = page.locator(groupHeading);
  if ((await heading.count()) && (await heading.getAttribute('aria-expanded')) === 'false') await heading.click();
  await expect(page.locator(item)).toBeVisible();
}

test('a window menu item opens a dialog without leaving the page', async ({ page }) => {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/ui/card');
  const url = page.url();
  await reveal(page);
  const button = page.locator(item);
  await expect(button).not.toHaveAttribute('href');
  await expect(button).toHaveAttribute('aria-haspopup', 'dialog');
  await button.click();
  const dialog = page.locator('dialog.z-dialog[open]');
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'About this catalog' })).toBeVisible();
  expect(page.url()).toBe(url);
  await page.keyboard.press('Escape');
  await expect(page.locator('dialog[open]')).toHaveCount(0);
  expect(page.url()).toBe(url);
  const toggle = page.locator('[data-zeyn-nav-toggle]');
  await expect(await toggle.isVisible() ? toggle : button).toBeFocused();
  expect(errors).toEqual([]);
});

test('the keyboard reaches and runs a window menu item', async ({ page, isMobile }) => {
  test.skip(isMobile, 'keys of the side menu are a desktop matter');
  await page.goto('/ui/card');
  await reveal(page);
  const all = page.locator('.z-nav .z-nav-group:not(.z-nav-favorites) a[href]').filter({ hasText: 'All components' });
  await all.focus();
  await page.keyboard.press('ArrowDown');
  const button = page.locator(item);
  await expect(button).toBeFocused();
  expect(await button.evaluate((b) => b.matches(':focus-visible') && getComputedStyle(b).outlineStyle !== 'none')).toBe(true);
  await page.keyboard.press('Enter');
  await expect(page.locator('dialog.z-dialog[open]')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(button).toBeFocused();
  await page.keyboard.press('Space');
  await expect(page.locator('dialog.z-dialog[open]')).toBeVisible();
  await page.keyboard.press('Escape');
});

test('the icon rail closes its flyout and gives focus to its heading', async ({ page, isMobile }) => {
  test.skip(isMobile, 'the rail exists on wide screens only');
  await page.goto('/ui/card');
  await page.locator('[data-zeyn-rail-toggle]').click();
  const heading = page.locator(groupHeading);
  await heading.click();
  await page.locator(item).click();
  await expect(page.locator('dialog.z-dialog[open]')).toBeVisible();
  await expect(heading).toHaveAttribute('aria-expanded', 'false');
  await page.keyboard.press('Escape');
  await expect(heading).toBeFocused();
  await page.locator('[data-zeyn-rail-toggle]').click();
});

test('the command palette lists a window menu item and opens the same dialog', async ({ page, isMobile }) => {
  test.skip(isMobile, 'the palette opens from the keyboard');
  await page.goto('/ui/card');
  const url = page.url();
  await page.keyboard.press('Alt+Shift+P');
  const input = page.locator('.z-palette-input');
  await expect(input).toBeFocused();
  await input.fill('about this cat');
  await expect(page.locator('.z-palette-list [role="option"]').first()).toContainText('About this catalog');
  await page.keyboard.press('Enter');
  await expect(page.locator('dialog.z-dialog[open]').getByRole('heading', { name: 'About this catalog' })).toBeVisible();
  expect(page.url()).toBe(url);
  await page.keyboard.press('Escape');
  await expect(page.locator('dialog[open]')).toHaveCount(0);
});
