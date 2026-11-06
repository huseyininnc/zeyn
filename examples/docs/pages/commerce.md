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
