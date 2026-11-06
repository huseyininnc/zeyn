# Shop components

Product listings and product pages need a few components of their own: prices with discounts, filters with counts, a range slider, variant pickers, a gallery, quantity steppers, review summaries and countdowns. They are server rendered like every other component. Those that need browser code load a small module only on pages that hold them, and their styles (`commerce.css`) are linked only there too.

## Price

```rust
Price::new(p.price)
    .was(p.list_price)
    .unit(p.price_per_kg, PriceUnit::Kg)
    .large()
```

- **Discount.** `was(old)` strikes the old price through next to the current one and shows the percent off, rounded to a whole number. An old price that is not higher than the current one, or in another currency, is left out, so a stale discount never shows a negative percent.
- **Screen readers** hear one sentence, "Was ₺1,599.00, now ₺1,299.00, 19% off". The struck price and the badge are hidden from them, because most screen readers do not say that text is struck through.
- **Unit price.** `unit(price, PriceUnit::Kg)` adds a quieter line such as "₺259.80 per kg". The units are `Item`, `Kg`, `Grams100`, `Liter`, `Ml100` and `Meter`.
- **Format.** Amounts are `Money` in the page's number format: `₺1,299.00` in English, `1.299,00 ₺` in Turkish, and the percent sign goes first where the language puts it first (`%19`). `large()` makes the current price bigger for a product page.
- **No script.** The price is plain HTML.

## Range slider

```rust
RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
    .step(50)
    .money(Currency::Try)
    .value(p.min_price, p.max_price)
    .drives("products")
```

- **Two native handles.** Each handle is an `<input type="range">` carrying one of the two names, so the browser gives it the W3C APG slider behavior: one tab stop each, arrow keys move by the step, Home and End go to the ends, and screen readers hear the formatted value (`aria-valuetext`, such as "₺500.00") and the label ("Price, from", "Price, to"). Page Up and Page Down move by a tenth of the range.
- **A filter.** `.drives(topic)` wraps the slider in a filter form: when a handle is let go, the two values go into the screen params of the same names and the listeners of the topic refresh in place. Inside a facet panel the panel's form does that, and in any other form the handles are sent like other fields.
- **Values.** `value(low, high)` takes the params (`None` keeps the end of the track). Values out of the bounds are clamped and a low value above the high one is swapped, so a hand edited address cannot break the slider. `money(currency)` shows whole units of a currency in the page's format.
- **Dates.** `RangeSlider::dates(label, names, first, last)` with `.date_value(from, to)` picks days; the form sends ISO dates such as `2026-03-31` through hidden inputs.
- **Script.** `range.js` (about 1 KB gzip) loads only on pages with a slider. It keeps the low handle at or below the high one, fills the track between the handles, writes the values next to the label and moves the nearer handle when the bare track is pressed. It reads the track's size once per press and never while a handle moves.
