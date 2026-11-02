# Theming

Status: implemented in `crates/zeyn/assets/zeyn.css`.

The default UI kit reads every color, font, size and radius from CSS custom properties on `:root`. An organization changes the look by overriding a few of them. It never needs to edit `zeyn.css` or write selectors.

## 1. How a theme is applied

The server serves `zeyn.css` first and then, when the app has one, an extra stylesheet with token overrides. The override file only contains `:root` blocks:

```css
:root {
  --z-brand: #7a2448;
  --z-font: "Source Sans 3", "Segoe UI", system-ui, sans-serif;
  --z-radius: 8px;
  --z-density: 0.75;
}
```

Rules for the override file:

- It must be served from the app's own origin. The Content Security Policy is `style-src 'self'`, so inline `<style>` blocks and third party stylesheets are blocked.
- Fonts must already be installed on the users' machines or be served from the app's origin. The kit itself loads no web fonts.
- Load it after `zeyn.css`, so equal selectors win by order.
- Set the base tokens in section 2. The derived tokens in section 3 follow automatically. Override a derived token only when the computed value does not work for your brand.

### Light and dark

The theme follows the operating system setting. Two attributes on `<html>` force one theme:

| Markup | Result |
|---|---|
| `<html>` | Follows `prefers-color-scheme` |
| `<html data-theme="light">` | Always light |
| `<html data-theme="dark">` | Always dark |

To give the dark theme its own values, repeat the selectors the kit uses:

```css
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --z-bg: #0e1012; }
}
:root[data-theme="dark"] { --z-bg: #0e1012; }
```

### Density

`--z-density` scales the spacing unit. `1` is regular, `0.75` is compact. `<html data-density="compact">` sets `0.75` without a stylesheet. Controls and grid rows never get smaller than 24 px, so compact mode still meets the WCAG 2.2 target size.

The row height of a virtual grid is fixed by the server (`Paging::virtual_rows(px)`, 32 px by default) because the runtime computes scroll positions from it. Density does not change it.

## 2. Base tokens

These are the tokens an organization is expected to set.

| Token | Purpose | Light | Dark |
|---|---|---|---|
| `--z-brand` | The one brand color. Primary buttons, the app name band, current menu item, focused row, focus ring | `#0f6674` | same value; see `--z-accent` |
| `--z-on-brand` | Text on a solid `--z-brand` surface | `#ffffff` | `#ffffff` |
| `--z-font` | Interface font stack | `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, "Noto Sans", sans-serif` | same |
| `--z-font-mono` | Code, API examples | `ui-monospace, "Cascadia Mono", "SF Mono", "Segoe UI Mono", Menlo, Consolas, monospace` | same |
| `--z-font-size` | Body text | `0.875rem` (14 px) | same |
| `--z-font-size-sm` | Grid cells, labels, filters | `0.8125rem` (13 px) | same |
| `--z-font-size-xs` | Column headers, menu group titles, page info | `0.75rem` (12 px) | same |
| `--z-radius` | Base corner radius, 0 to 12 px. Every component scales from it | `4px` | same |
| `--z-density` | Spacing multiplier, `1` regular, `0.75` compact | `1` | same |
| `--z-bg` | Page background behind panels | `#f4f5f6` | `#121416` |
| `--z-surface` | Panels, grids, windows, inputs | `#ffffff` | `#1a1d20` |
| `--z-surface-2` | Grid header, zebra rows, window footer, read only inputs | `#f7f8f9` | `#202326` |
| `--z-hover` | Hover background for rows, menu items, buttons | `#eceef1` | `#2a2e33` |
| `--z-border` | Dividers and panel outlines | `#dcdfe3` | `#33383e` |
| `--z-border-strong` | Input and button outlines, header rule. Must reach 3:1 against `--z-surface` | `#7d848d` | `#7a818a` |
| `--z-text` | Body text and values | `#1a1d21` | `#e4e6e9` |
| `--z-text-muted` | Labels, headers, secondary text. Must reach 4.5:1 against `--z-surface-2` | `#5b626b` | `#a2a9b1` |
| `--z-danger` | Errors, required marks, danger rows | `#b3261e` | `#f28b82` |
| `--z-warning` | Warning rows | `#8a5300` | `#e6b45a` |
| `--z-success` | Success rows | `#1e6b3a` | `#6fcf8f` |
| `--z-info` | Info toasts | `#2b5d8a` | `#8ab8e6` |

## 3. Derived tokens

Computed from the base tokens with `color-mix()` and relative color syntax. They update by themselves when a base token changes.

| Token | Purpose | Light | Dark |
|---|---|---|---|
| `--z-accent` | Brand color used as text or a thin line on a surface: links, current menu item, focused row bar | `var(--z-brand)` | `oklch(from var(--z-brand) 0.8 c h)`, the brand hue at a fixed light tone. Browsers without relative color syntax get `color-mix(in oklab, var(--z-brand) 45%, #fff)` |
| `--z-primary` | Primary button fill | `var(--z-brand)` | `var(--z-accent)` |
| `--z-on-primary` | Primary button text | `var(--z-on-brand)` | `#121416` |
| `--z-primary-hover` | Primary button hover | brand 86% mixed with black | accent 84% mixed with white |
| `--z-primary-active` | Primary button pressed | brand 74% mixed with black | accent 70% mixed with white |
| `--z-brand-subtle` | Tint behind the current menu item and the focused grid row | accent at `--z-tint` over `--z-surface` | same formula |
| `--z-focus` | Focus ring | `var(--z-accent)` | `var(--z-accent)` |
| `--z-danger-subtle`, `--z-warning-subtle`, `--z-success-subtle`, `--z-info-subtle` | Row and message backgrounds | status color at `--z-tint` over `--z-surface` | same formula |
| `--z-tint` | Strength of the subtle tints | `9%` | `20%` |
| `--z-space` | Spacing unit | `calc(4px * var(--z-density))` | same |
| `--z-control` | Button and input height | `max(24px, 8 * --z-space)` (32 px) | same |
| `--z-row` | Grid row height for paged and infinite grids | `max(24px, 8 * --z-space)` (32 px) | same |
| `--z-gap` | Gap between screen sections | `3 * --z-space` (12 px) | same |
| `--z-radius-sm`, `--z-radius-lg` | Small parts (filters, code), windows | `0.75x` and `1.5x` of `--z-radius` | same |
| `--z-shadow-2` | Windows and toasts only. Grid rows never get a shadow | soft two layer shadow | darker |
| `--z-backdrop` | Behind a modal window | `rgb(20 24 28 / 40%)` | `rgb(0 0 0 / 60%)` |

Per element tokens set by the server markup: `--z-width` (window width), `--z-flex` (column share), `--z-row-height` (virtual grid row height), and for the component library `--z-stack-gap`, `--z-split`, `--z-progress`, `--z-columns`, `--z-level` and `--z-pop-x` / `--z-pop-y` (popup position).

Chart series colors are `--z-chart-1` to `--z-chart-6` in `components.css`, with lighter values in the dark theme. Each reaches 3:1 against `--z-surface`. Override them like any base token.

## 4. Choosing a brand color

- `--z-brand` carries white text in the light theme (buttons, app name band). Pick a color with at least 4.5:1 against `--z-on-brand`. Most mid to dark brand colors pass. For a light brand color such as yellow, also set `--z-on-brand` to a dark color.
- In the dark theme the kit lightens the brand for text, rings and primary buttons, and keeps the solid brand for the app name band. A different brand looks deliberate in both themes without a second color.
- Keep status colors distinct from the brand. A red brand next to red error rows makes errors harder to spot; in that case shift `--z-danger` toward a different red or keep the brand off grids by overriding `--z-brand-subtle`.
- Check your colors with the table in section 5 as a target. Any pair below the threshold fails WCAG 2.2 AA.

## 5. Verified contrast

Measured in Chromium on the running example app: every token was resolved to its rendered sRGB value (including `color-mix()` and relative colors), then the WCAG 2.x contrast ratio was computed. Thresholds: 4.5:1 for text, 3:1 for UI component outlines and focus indicators.

| Pair | Light | Dark | Needed |
|---|---|---|---|
| `--z-text` on `--z-bg` | 15.49 | 14.77 | 4.5 |
| `--z-text` on `--z-surface` | 16.91 | 13.54 | 4.5 |
| `--z-text` on `--z-surface-2` | 15.91 | 12.63 | 4.5 |
| `--z-text` on `--z-hover` | 14.55 | 10.93 | 4.5 |
| `--z-text` on `--z-brand-subtle` (focused row) | 14.79 | 9.27 | 4.5 |
| `--z-text` on `--z-danger-subtle` | 14.73 | 9.76 | 4.5 |
| `--z-text` on `--z-warning-subtle` | 14.82 | 9.31 | 4.5 |
| `--z-text` on `--z-success-subtle` | 14.76 | 9.33 | 4.5 |
| `--z-text-muted` on `--z-bg` | 5.65 | 7.78 | 4.5 |
| `--z-text-muted` on `--z-surface` | 6.17 | 7.13 | 4.5 |
| `--z-text-muted` on `--z-surface-2` (headers) | 5.80 | 6.65 | 4.5 |
| `--z-text-muted` on `--z-hover` | 5.31 | 5.76 | 4.5 |
| `--z-text-muted` on `--z-danger-subtle` (muted row) | 5.37 | 5.14 | 4.5 |
| `--z-text-muted` on `--z-warning-subtle` | 5.41 | 4.90 | 4.5 |
| `--z-accent` on `--z-surface` (links) | 6.62 | 9.33 | 4.5 |
| `--z-accent` on `--z-bg` | 6.06 | 10.18 | 4.5 |
| `--z-accent` on `--z-brand-subtle` (current menu item) | 5.79 | 6.39 | 4.5 |
| `--z-on-primary` on `--z-primary` | 6.62 | 10.18 | 4.5 |
| `--z-on-primary` on `--z-primary-hover` | 8.82 | 11.26 | 4.5 |
| `--z-on-primary` on `--z-primary-active` | 11.12 | 12.26 | 4.5 |
| `--z-on-brand` on `--z-brand` (app name band) | 6.62 | 6.62 | 4.5 |
| `--z-danger` on `--z-surface` (field errors) | 6.54 | 7.09 | 4.5 |
| `--z-danger` on `--z-danger-subtle` | 5.69 | 5.11 | 4.5 |
| `--z-warning` on `--z-surface` | 6.33 | 8.90 | 4.5 |
| `--z-warning` on `--z-warning-subtle` | 5.55 | 6.12 | 4.5 |
| `--z-success` on `--z-surface` | 6.52 | 8.87 | 4.5 |
| `--z-info` on `--z-surface` | 6.92 | 8.13 | 4.5 |
| `--z-focus` on `--z-surface` | 6.62 | 9.33 | 3 |
| `--z-focus` on `--z-bg` | 6.06 | 10.18 | 3 |
| `--z-focus` on `--z-brand-subtle` | 5.79 | 6.39 | 3 |
| `--z-border-strong` on `--z-surface` (input outline) | 3.78 | 4.30 | 3 |
| `--z-border-strong` on `--z-surface-2` | 3.55 | 4.01 | 3 |

`--z-border` is decorative (dividers between items that already have text) and is not held to 3:1.

## 6. Other accessibility behavior

- Every interactive element shows a 2 px `:focus-visible` ring in `--z-focus`. Grid rows draw the ring inside the row so it is never clipped by the scroll container.
- Buttons, inputs, sort headers, filter boxes, pager buttons and the window close button are at least 24 x 24 px at any density.
- `prefers-reduced-motion: reduce` removes the toast entrance and all transitions.
- In `forced-colors` mode icons use `CanvasText`, the primary button and the focused row use `Highlight`, and state that relies on background tints (current menu item, row status) gets an outline or border instead.
