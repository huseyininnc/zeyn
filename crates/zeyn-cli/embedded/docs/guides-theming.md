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

### Light, dark and app themes

Every app has a theme picker in the menu, next to the language picker. It offers System (follows `prefers-color-scheme`), Light, Dark and any theme the app defines:

```rust
App::new("Billing")
    .theme(Theme::new().brand("#7a2448").radius(6))
    .themes([
        Theme::light(),
        Theme::dark(),
        Theme::named("sand", [("--z-bg", "#f5f1ea"), ("--z-surface", "#fffdf8")]).label(t!("theme.sand")),
    ])
    .default_theme("system")
```

- `App::theme` is the organization's look and applies under every theme.
- `App::themes` lists what users can choose. Without it the list is Light and Dark. System is offered when both are in the list. `Theme::named(...)` builds on the light theme; `.dark_base()` builds it on the dark one.
- `App::default_theme` is the theme of users who have not chosen. `system` unless set.
- `App::theme_picker(false)` hides the picker; everyone gets the default theme.
- A single theme locks the app: `.themes([Theme::named("acme", tokens)])` or `.theme(Theme::dark())`.

Tokens are checked when the app starts. `App::check` (and so `router` and `serve`) reports unknown token names, values that could leave their declaration (`;`, braces, `\`, `<`, `>`, `url(`), bad theme names, an unknown default theme, and every color pair of section 5 that a theme pushes below WCAG 2.2 AA. Only `#rgb` and `#rrggbb` values can be checked for contrast; other color syntax is trusted.

All themes are served in one stylesheet, `/_zeyn/theme.css`, built once at startup and loaded after the kit's stylesheets. The choice is stored in the `zeyn_theme` cookie and, for a signed in user in an app with a view store (`App::datagrid_views`), in that store, so it follows the user to a new device. The server renders the theme on `<html>`, so the first paint is already right and no inline script is needed:

| Markup | Result |
|---|---|
| `<html>` | System: follows `prefers-color-scheme` |
| `<html data-theme="light">` | Always light |
| `<html data-theme="dark">` | Always dark |
| `<html data-theme="light" data-zeyn-theme="sand">` | The app theme `sand`, on the light theme |

Changing the theme in the picker only changes these attributes and the `color-scheme` meta, then calls `POST /_zeyn/theme`. Nothing reloads.

To give the dark theme its own values in a hand written override file, repeat the selectors the kit uses:

```css
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --z-bg: #0e1012; }
}
:root[data-theme="dark"] { --z-bg: #0e1012; }
```

### Density

Two densities, switched by tokens. Comfortable is the default: 40 px grid rows, 36 px headers, 36 px controls, 20 px card padding. Compact (`<html data-density="compact">`) has 32 px rows, headers and controls, and scales the spacing unit to 0.8. Users pick it in the account menu of the top bar (`App::header(|h| h.account())`); the choice is the `zeyn_density` cookie, which the server renders on `<html>`, so the first paint is right. Controls and grid rows never get smaller than 24 px, so compact still meets the WCAG 2.2 target size. `Theme::new().compact()` makes compact spacing the organization's default.

The row height of a virtual grid is fixed by the server because the runtime computes scroll positions from it. `Paging::virtual_scroll()` follows the density (40 px comfortable, 32 px compact); `Paging::virtual_rows(px)` keeps the height it is given. A density change applies to virtual grids from the next page.

## 2. Base tokens

These are the tokens an organization is expected to set.

| Token | Purpose | Light | Dark |
|---|---|---|---|
| `--z-brand` | The one brand color. Logo mark, primary buttons, current menu item, focus ring | `#0f6674` | same value; see `--z-accent` |
| `--z-on-brand` | Text on a solid `--z-brand` surface | `#ffffff` | `#ffffff` |
| `--z-font` | Interface font stack | `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, "Noto Sans", sans-serif` | same |
| `--z-font-mono` | Code, API examples | `ui-monospace, "Cascadia Mono", "SF Mono", "Segoe UI Mono", Menlo, Consolas, monospace` | same |
| `--z-font-size` | Body text | `0.875rem` (14 px) | same |
| `--z-font-size-sm` | Grid cells, labels, filters | `0.8125rem` (13 px) | same |
| `--z-font-size-xs` | Column headers, menu group titles, page info | `0.75rem` (12 px) | same |
| `--z-radius` | Base corner radius, 0 to 12 px. Buttons, inputs, menu items; every other radius derives from it | `6px` | same |
| `--z-density` | Spacing multiplier, `1` regular, `0.75` compact | `1` | same |
| `--z-bg` | Page background behind panels | `#f5f6f8` | `#0d0f12` |
| `--z-surface` | Panels, grids, cards, inputs | `#ffffff` | `#15181d` |
| `--z-surface-2` | Grid header and footer, window footer, read only inputs | `#f8f9fa` | `#1a1e24` |
| `--z-raised` | Popovers, menus, dialogs, toasts, the command palette. In dark, raised layers are lighter instead of relying on shadows | `#ffffff` | `#1f242b` |
| `--z-hover` | Hover background for rows, menu items, buttons | `#eff1f4` | `#232830` |
| `--z-border` | Dividers and panel outlines | `#e2e5e9` | `#2b3139` |
| `--z-border-strong` | Input outlines, checkbox borders, chart baselines. Must reach 3:1 against `--z-surface` | `#858d97` | `#6f7884` |
| `--z-text` | Body text and values | `#15181d` | `#e7eaee` |
| `--z-text-muted` | Labels, headers, secondary text. Must reach 4.5:1 against `--z-surface-2` | `#5a626c` | `#9ba4af` |
| `--z-danger` | Errors, required marks, danger rows | `#b42318` | `#f28b82` |
| `--z-warning` | Warning rows | `#8a5300` | `#e6b45a` |
| `--z-success` | Success rows | `#1a6b3c` | `#6fcf8f` |
| `--z-info` | Info toasts | `#275d93` | `#8ab8e6` |

## 3. Derived tokens

Computed from the base tokens with `color-mix()` and relative color syntax. They update by themselves when a base token changes.

| Token | Purpose | Light | Dark |
|---|---|---|---|
| `--z-accent` | Brand color used as text or a thin line on a surface: links, current menu item, focused row bar | `var(--z-brand)` | `oklch(from var(--z-brand) 0.8 c h)`, the brand hue at a fixed light tone. Browsers without relative color syntax get `color-mix(in oklab, var(--z-brand) 45%, #fff)` |
| `--z-primary` | Primary button fill | `var(--z-brand)` | `var(--z-accent)` |
| `--z-on-primary` | Primary button text | `var(--z-on-brand)` | `#0d0f12` |
| `--z-primary-hover` | Primary button hover | brand 86% mixed with black | accent 84% mixed with white |
| `--z-primary-active` | Primary button pressed | brand 74% mixed with black | accent 70% mixed with white |
| `--z-brand-subtle` | Tint behind the current menu item and the focused grid row | accent at `--z-tint` over `--z-surface` | same formula |
| `--z-focus` | Focus ring | `var(--z-accent)` | `var(--z-accent)` |
| `--z-danger-subtle`, `--z-warning-subtle`, `--z-success-subtle`, `--z-info-subtle` | Row and message backgrounds | status color at `--z-tint` over `--z-surface` | same formula |
| `--z-tint` | Strength of the subtle tints | `9%` | `16%` |
| `--z-text-2xs` to `--z-text-num-lg` with `--z-leading-*` | Type scale, size over line height: `2xs` 11/16 (keys, chart ticks on phones), `xs` 12/16 (grid headers, captions, badges), `sm` 13/20 (cells, nav, labels, buttons), `md` 14/20 (body, inputs), `lg` 16/24 (card, chart and dialog section titles), `xl` 20/28 (window titles, page titles on phones), `2xl` 24/32 (page title), `num` 30/36 (KPI values), `num-lg` 40/44 (hero metric). Weights are 400 for text, 500 for labels, buttons and current items, 600 for titles and numbers only | | same |
| `--z-space` | Spacing unit | `calc(4px * var(--z-density))` | same |
| `--z-control` | Button and input height | `max(24px, 9 * --z-space)` (36 px; 32 px compact) | same |
| `--z-row`, `--z-row-head` | Grid row and header height for paged and infinite grids | 40 px and 36 px (32 px compact) | same |
| `--z-gap` | Gap between cards and screen sections | `4 * --z-space` (16 px) | same |
| `--z-s-0_5`, `--z-s-1`, `--z-s-1_5`, `--z-s-2`, `--z-s-3`, `--z-s-4`, `--z-s-5`, `--z-s-6`, `--z-s-8`, `--z-s-10`, `--z-s-12`, `--z-s-16` | Spacing scale: the number times `--z-space` (2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48 and 64 px at density 1) | | same |
| `--z-radius-sm` | Badges, chips, checkboxes, keyboard keys | `max(2px, --z-radius - 2px)` (4 px) | same |
| `--z-radius-lg` | Cards, grids, popovers, toasts | `--z-radius + 2px` (8 px) | same |
| `--z-radius-xl` | Dialogs, the sign in card, the command palette | `--z-radius + 6px` (12 px) | same |
| `--z-radius-full` | Avatars, switches, status dots, count pills | `999px` | same |
| `--z-shadow-1` | Default buttons, the thumb of a segmented control | `0 1px 2px` at 6% | none |
| `--z-shadow-2` | Popovers, menus, tooltips, toasts. Grid rows and cards never get a shadow | two soft layers | a 1 px `--z-border` ring and a deep shadow |
| `--z-shadow-3` | Dialogs and the command palette | two deeper layers | a 1 px ring and a deeper shadow |
| `--z-dur-1`, `--z-dur-2`, `--z-dur-3`, `--z-dur-out` | Motion: hover colors (80 ms), popovers and toasts in (140 ms), dialogs, drawers and the palette in (200 ms), every exit (100 ms). Under `prefers-reduced-motion: reduce` they become 0, fades 80 ms at most | | same |
| `--z-ease-out`, `--z-ease-in`, `--z-ease-linear` | Easing of entries and moves, of exits, and of color changes | `cubic-bezier(0.2, 0, 0, 1)`, `cubic-bezier(0.4, 0, 1, 1)`, `linear` | same |
| `--z-backdrop` | Behind a modal window | `rgb(20 24 28 / 40%)` | `rgb(0 0 0 / 60%)` |
| `--z-dir` | Reading direction as a sign, for offsets with no logical form (inset box shadows that mark the start or end side, slide transforms): `calc(2px * var(--z-dir))` | `1`, and `-1` under `dir="rtl"` | same |

Per element tokens set by the server markup: `--z-width` (window width), `--z-flex` (column share), `--z-row-height` (virtual grid row height), and for the component library `--z-stack-gap`, `--z-split`, `--z-progress`, `--z-columns`, `--z-level` and `--z-pop-x` / `--z-pop-y` (popup position).

Chart series colors are `--z-chart-1` to `--z-chart-8` in `components.css`: blue `#2563d9`, orange `#d67400`, magenta `#a3369b`, green `#14866d`, olive `#7a6a00`, violet `#6e56cf`, red `#cc3b33` and slate `#56636f`, with lighter values in the dark theme (`#5b9bff`, `#f08a3c`, `#e27bd6`, `#38c29f`, `#ece27a`, `#9d8cff`, `#ff7b70`, `#9aa7b5`). The order keeps the first five apart under deuteranopia, protanopia and tritanopia, and series 1 is not the brand, so a red or green brand never reads as good or bad; an app may still set `--z-chart-1: var(--z-brand)`. The startup check holds each to 3:1 against `--z-surface`. Override them like any base token.

## 4. Choosing a brand color

- `--z-brand` carries white text in the light theme (primary buttons, the logo letter mark, the brand panel of the sign in shell). Pick a color with at least 4.5:1 against `--z-on-brand`. Most mid to dark brand colors pass. For a light brand color such as yellow, also set `--z-on-brand` to a dark color.
- In the dark theme the kit lightens the brand for text, rings and primary buttons, and keeps the solid brand for the logo letter mark. A different brand looks deliberate in both themes without a second color.
- Keep status colors distinct from the brand. A red brand next to red error rows makes errors harder to spot; in that case shift `--z-danger` toward a different red or keep the brand off grids by overriding `--z-brand-subtle`.
- Check your colors with the table in section 5 as a target. Any pair below the threshold fails WCAG 2.2 AA.

## 5. Verified contrast

Every pair below is checked when the app starts (`App::check`), for the built in themes and for every app theme, and by the unit test `built_in_themes_reach_wcag_aa_on_every_pair` in `src/server/theme.rs` (run it with `--nocapture` to print this table). Derived tokens (`--z-accent`, the primary colors and the subtle tints) are computed the way `zeyn.css` computes them, in Oklab. Thresholds: 4.5:1 for text, 3:1 for UI component outlines and focus indicators.

| Pair | Light | Dark | Needed |
|---|---|---|---|
| `--z-text` on `--z-bg` | 16.45 | 15.90 | 4.5 |
| `--z-text` on `--z-surface` | 17.79 | 14.74 | 4.5 |
| `--z-text` on `--z-surface-2` | 16.88 | 13.86 | 4.5 |
| `--z-text` on `--z-raised` | 17.79 | 12.93 | 4.5 |
| `--z-text` on `--z-hover` | 15.72 | 12.27 | 4.5 |
| `--z-text` on `--z-brand-subtle` | 15.56 | 11.18 | 4.5 |
| `--z-text` on `--z-danger-subtle` | 15.49 | 11.60 | 4.5 |
| `--z-text` on `--z-warning-subtle` | 15.59 | 11.20 | 4.5 |
| `--z-text` on `--z-success-subtle` | 15.53 | 11.23 | 4.5 |
| `--z-text` on `--z-info-subtle` | 15.52 | 11.40 | 4.5 |
| `--z-text-muted` on `--z-bg` | 5.71 | 7.61 | 4.5 |
| `--z-text-muted` on `--z-surface` | 6.18 | 7.05 | 4.5 |
| `--z-text-muted` on `--z-surface-2` | 5.86 | 6.63 | 4.5 |
| `--z-text-muted` on `--z-raised` | 6.18 | 6.19 | 4.5 |
| `--z-text-muted` on `--z-hover` | 5.46 | 5.87 | 4.5 |
| `--z-text-muted` on `--z-brand-subtle` | 5.40 | 5.35 | 4.5 |
| `--z-border-strong` on `--z-surface` | 3.36 | 3.98 | 3 |
| `--z-border-strong` on `--z-surface-2` | 3.19 | 3.74 | 3 |
| `--z-border-strong` on `--z-raised` | 3.36 | 3.49 | 3 |
| `--z-accent` on `--z-surface` | 6.62 | 9.81 | 4.5 |
| `--z-accent` on `--z-bg` | 6.12 | 10.58 | 4.5 |
| `--z-accent` on `--z-brand-subtle` | 5.79 | 7.44 | 4.5 |
| `--z-focus` on `--z-surface` | 6.62 | 9.81 | 3 |
| `--z-focus` on `--z-bg` | 6.12 | 10.58 | 3 |
| `--z-on-primary` on `--z-primary` | 6.62 | 10.58 | 4.5 |
| `--z-on-primary` on `--z-primary-hover` | 8.82 | 11.70 | 4.5 |
| `--z-on-primary` on `--z-primary-active` | 11.12 | 12.74 | 4.5 |
| `--z-danger` on `--z-surface` | 6.57 | 7.45 | 4.5 |
| `--z-danger` on `--z-danger-subtle` | 5.73 | 5.86 | 4.5 |
| `--z-warning` on `--z-surface` | 6.33 | 9.35 | 4.5 |
| `--z-warning` on `--z-warning-subtle` | 5.55 | 7.11 | 4.5 |
| `--z-success` on `--z-surface` | 6.54 | 9.32 | 4.5 |
| `--z-success` on `--z-success-subtle` | 5.70 | 7.09 | 4.5 |
| `--z-info` on `--z-surface` | 6.83 | 8.54 | 4.5 |
| `--z-info` on `--z-info-subtle` | 5.96 | 6.60 | 4.5 |
| `--z-on-brand` on `--z-brand` | 6.62 | 6.62 | 4.5 |
| `--z-brand` on `--z-surface` | 6.62 | n.a. | 4.5 |

`--z-border` is decorative (dividers between items that already have text) and is not held to 3:1.

## 6. Other accessibility behavior

- Every interactive element shows a 2 px `:focus-visible` ring in `--z-focus`. Grid rows draw the ring inside the row so it is never clipped by the scroll container.
- Buttons, inputs, sort headers, filter boxes, pager buttons and the window close button are at least 24 x 24 px at any density.
- `prefers-reduced-motion: reduce` removes the toast entrance and all transitions.
- In `forced-colors` mode icons use `CanvasText`, the primary button and the focused row use `Highlight`, and state that relies on background tints (current menu item, row status) gets an outline or border instead.
