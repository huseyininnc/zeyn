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


### Prices on cards

`ImageCard::price(Price)` puts the same price under the card's text in a `CardGrid` or `Rail`, inside the card's link, so screen readers hear the title and the price sentence together. It adds no script; the shop stylesheet comes with the first card that has a price.

```rust
CardGrid::of(catalog_products).card(|p: &Product| {
    Card::image(p.photo.clone(), p.name.clone())
        .price(Price::new(p.price).was(p.list_price))
        .link(product_page, ProductPage { id: p.id })
})
```

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

## Quantity stepper

```rust
#[action(permission = "cart.edit")]
pub async fn set_quantity(change: QuantityChange, cart: CartStore) -> Result<QuantityState> {
    let stock = cart.stock(&change.target)?;
    let kept = change.quantity.min(stock);
    cart.set(&change.target, kept)?;
    Ok(QuantityState::new(kept).max(stock))
}

QuantityStepper::of(set_quantity, line.sku.clone())
    .label(t!("cart.quantity"))
    .range(1, 20)
    .value(line.quantity)
```

- **Checked on the server.** A change (a press of minus or plus, or a typed number) is sent 400 ms after the user stops, through `POST /_zeyn/data` with the CSRF check and the write rate limit, as the user, with the action's permission checked. The server clamps the quantity to the stepper's range before the action runs, and the action checks what only it knows, such as the stock. The item (`target`) is set by the screen and never read from the browser.
- **The answer wins.** The action answers with the quantity it kept and the most allowed (`QuantityState::new(kept).max(stock)`). When that is less than the user asked for, the stepper shows it and says "Only 7 available." in a live region. A newer change cancels the request still on its way, so only the last answer is shown; a failed request puts the last kept quantity back and says so.
- **Keyboard and touch.** The buttons are at least 24 by 24 pixels (WCAG 2.5.8), named "Decrease" and "Increase", and turn off at the ends of the range; focus moves to the field when the pressed button turns off. The field takes digits and the arrow keys.
- **Script.** `quantity.js` (about 1 KB gzip) loads only on pages with a stepper.

## Variant picker

```rust
VariantPicker::new()
    .group(
        VariantGroup::swatches("color", t!("shop.color"))
            .swatch("sand", t!("shop.sand"), "#d8c3a5")
            .swatch("navy", t!("shop.navy"), "#1f3a68")
            .current(p.color.as_deref()),
    )
    .group(
        VariantGroup::chips("size", t!("shop.size"))
            .option("m", "M")
            .option("xl", "XL")
            .unavailable("xl", t!("shop.not_in_color"))
            .current(p.size.as_deref()),
    )
    .drives("variant")
```

- **Radio groups.** Each group is a fieldset of native radio buttons, the W3C APG radio group pattern without script: Tab reaches the chosen option, the arrow keys move and choose, and the legend says the group and the chosen option ("Color Navy"). Swatches take a hex color or a theme token; chips show their text.
- **Unavailable options** are turned off with a reason (`unavailable(value, reason)`), which screen readers read with the option and the tooltip shows; swatches get a diagonal line and chips a strike through.
- **Price, photo and stock follow.** The picker drives its topics and listens to them itself. Choosing an option puts its value into the screen params of the group's name, keeps the address in step, so the link to the chosen variant can be shared, and refreshes every node that listens to the topic in one request: put `.listens("variant")` on the price, the gallery and the stock line, and the screen turns options off again for the new choice. It needs no script beyond the topics module.
- **Checks.** `App::check_page` reports a picker that drives nothing and a swatch without a valid color.

## Product gallery

```rust
p.photos.iter().fold(ProductGallery::new(p.name.clone()), |g, photo| {
    g.image(
        Image::new(photo.large.clone())
            .alt(photo.alt.clone())
            .size(800, 800)
            .low(photo.tiny.clone())
            .placeholder(Placeholder::color("--z-surface-2")),
    )
    .thumb(photo.thumb.clone())
})
```

- **One image at a time.** The slides sit in a scroll snapping track, so phones swipe between them without script. Thumbnails under the image are buttons ("Show photo 2 of 5", the current one with `aria-current`); the arrow keys on the focused image move between slides, in the reading direction of the page. The gallery is a named carousel region whose slides say "2 of 5" (W3C APG carousel pattern). Without thumbnails (`thumb`) it shows numbered dots.
- **Only what is seen loads.** The first image is eager with `fetchpriority="high"`, because it is usually the largest thing on a product page; the others wait in inert templates, and `gallery.js` puts in the image on screen and its two neighbors, watched by one IntersectionObserver. A gallery of 12 photos loads three at first. The images are `Image`s, so they keep their placeholder, low resolution file and fallback.
- **Zoom and full screen.** A mouse zooms the image under the pointer (the slide's box is read once when the pointer enters, never while it moves). A press on the image, or the "Full screen" button, opens a modal dialog with the image, previous and next buttons, a zoom toggle and a close button; it keeps focus inside, closes with Escape and gives focus back to what opened it. Under reduced motion nothing slides or grows.
- **Script.** `gallery.js` (about 1.5 KB gzip) loads only on pages with a gallery.

## Facet panel

```rust
#[params]
#[derive(Default)]
pub struct Listing {
    #[serde(default)]
    pub color: Vec<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
}

#[screen(route = "/shop", public)]
pub async fn shop(p: Listing, store: ProductStore) -> Result<Screen> {
    let colors = store.count_colors(&p).await?;
    let panel = FacetPanel::of(shop, &p)
        .facet(colors.iter().fold(Facet::new("color", t!("shop.color")), |f, c| {
            f.option(c.value.clone(), c.name.clone(), c.count)
        })
        .chosen(p.color.clone()))
        .range(RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
            .money(Currency::Try)
            .value(p.min_price, p.max_price))
        .drives("products");
    let cards = CardGrid::of(search_products)
        .card(|p: &Product| Card::image(p.photo.clone(), p.name.clone()).price(Price::new(p.price)))
        .scope_any(Product::color, p.color.clone())
        .scope_range(Product::price, p.min_price.map(|m| Money::tl(m * 100)), p.max_price.map(|m| Money::tl(m * 100)))
        .listens("products");
    Ok(Screen::new(t!("shop.title")).body(Row::new().col(panel).col(cards).weight(3)))
}
```

- **Counts from the server.** Each facet is a group of checkboxes whose options show how many rows they have with the other facets applied ("Red (124)"; screen readers hear "Red, 124 results"). The screen counts them with one bounded query per facet, such as `SELECT color, count(*) ... GROUP BY color LIMIT 50` with the other filters in the `WHERE`; a facet takes at most `MAX_FACET_OPTIONS` (50) options. An option with no rows is turned off unless it is chosen.
- **One request per change.** The panel is a filter form that drives its topics and listens to them itself. A change puts the values into the screen params (facets as lists, the range as two numbers), keeps the address in step and refreshes every listener in one `POST /_zeyn/nodes`: the card grid, a count line and the panel with its new counts. A newer change cancels the request still on its way. Measured on 2,000 products: 26 ms from a click to new cards and counts.
- **Filters stay on the server.** The grid takes the same params with `scope_any(field, values)` and `scope_range(field, from, to)` (on `CardGrid` and `DataGrid`), so the browser never sends filters of its own; no values or open ends keep every row.
- **Chosen filters** show as chips above the groups, with "Clear all". Each chip is a link to the screen without that filter, so it also works without script, and is named "Remove filter Red". The groups are `<details>` that fold, and open groups stay open across refreshes.
- **Checks.** `App::check_page` reports a panel that drives nothing and a facet whose name is not a field of the screen's params.

## Reviews and ratings

```rust
ReviewSummary::new()
    .count(5, stats.five)
    .count(4, stats.four)
    .count(3, stats.three)
    .count(2, stats.two)
    .count(1, stats.one)
```

- **Summary.** `ReviewSummary` shows the average as a number and as stars, the number of reviews and one bar per star from 5 to 1 with its share. The average and the shares come from the counts, so they always agree. Screen readers hear "4.6 out of 5 stars" and "5 stars, 68%" for each bar; the bars are hidden from them. It is HTML with no script.
- **Rating input.** `Control::Rating` on a form field is a radio group drawn as stars (W3C APG, no script). A whole number field takes whole stars; a `Decimal` field with `#[rule(min = 1, max = 5, scale = 1)]` takes half stars, each half a radio button named "4.5 of 5". The server reads the value in the page's number format and turns away a value between the halves, such as 4.3.

```rust
#[model]
pub struct Review {
    #[rule(required, min = 1, max = 5, scale = 1)]
    pub stars: Option<Decimal>,
    pub text: String,
}

Form::of(Review::default())
    .field(Review::stars)
    .control(Review::stars, Control::Rating)
    .field(Review::text)
    .submit(t!("reviews.send"), save_review)
```

## Countdown

```rust
Countdown::until(sale.ends_at)
    .label(t!("shop.sale_ends"))
    .ended(Badge::new(t!("shop.sale_over")))
```

- **Server time.** The server writes the time left with its own clock ("2d 03:14:09"), and the page carries that clock, so a wrong clock on the device does not move the end. Once the moment has passed, the server renders the end state given to `ended` instead of the clock, with no script.
- **One timer.** `countdown.js` (about 1 KB gzip, only on pages with a running countdown) keeps one timer for every countdown of the page, ticking on the whole second and stopped while the tab is hidden. At zero the end state replaces the clock; listeners of a topic can refresh what else the end changes.
- **Screen readers.** The digits are hidden from them. A polite live region says the time left in words ("2 d 3 h 14 min left") and changes only when the minute changes, so it never talks every second.
