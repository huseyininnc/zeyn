//! Input components: exact decimals, percentages, masked text and passwords.

use zeyn::prelude::*;

/// Payment terms of a supplier, edited with the decimal, percent, masked and password inputs.
#[model]
pub struct SupplierTerms {
    #[rule(required, max_len = 80)]
    pub supplier: String,
    #[rule(precision = 12, scale = 4, min = 0)]
    pub exchange_rate: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
    #[rule(scale = 3, min = 0, max = 100000)]
    pub minimum_order: Decimal,
    #[rule(required, mask = "(999) 999 99 99")]
    pub phone: String,
    #[rule(mask = "AA99 9999 9999 9999 9999 9999 99")]
    pub iban: Option<String>,
    #[rule(min_len = 10, max_len = 128)]
    pub portal_password: String,
}

/// Accepts valid terms. Nothing is stored.
#[action(public)]
pub async fn save_terms(terms: Valid<SupplierTerms>) -> Result<SupplierTerms> {
    Ok(terms.into_inner())
}

#[screen(route = "/fields", public)]
pub fn input_fields() -> Screen {
    let terms = SupplierTerms {
        supplier: "Anadolu Gıda".into(),
        exchange_rate: Decimal::parse("34.2175").unwrap_or_default(),
        discount: Decimal::parse("7.5"),
        minimum_order: Decimal::parse("250.000").unwrap_or_default(),
        phone: "2124445566".into(),
        iban: Some("TR330006100519786457841326".into()),
        ..SupplierTerms::default()
    };
    Screen::new(t!("inputs.fields")).body(
        Panel::new(t!("inputs.panel_terms")).add(
            Form::of(terms)
                .field(SupplierTerms::supplier)
                .row(|r| {
                    r.field(SupplierTerms::exchange_rate)
                        .field(SupplierTerms::discount)
                        .field(SupplierTerms::minimum_order)
                })
                .control(SupplierTerms::discount, Control::Percent)
                .row(|r| {
                    r.field(SupplierTerms::phone)
                        .field(SupplierTerms::iban)
                })
                .field(SupplierTerms::portal_password)
                .control(SupplierTerms::portal_password, Control::PasswordMeter)
                .submit(t!("inputs.save"), save_terms)
                .on_success(|_, ui| ui.toast(t!("inputs.saved"))),
        ),
    )
}

/// Ways a customer agrees to be reached.
#[choice]
pub enum Channel {
    Email,
    Phone,
    Sms,
    Post,
    Courier,
}

/// Regions a campaign runs in.
#[choice]
pub enum Region {
    Marmara,
    Aegean,
    Mediterranean,
    Central,
    BlackSea,
    East,
    Southeast,
}

/// A marketing campaign with several channels, regions and a product picked from columns.
#[model]
pub struct Campaign {
    #[rule(required, max_len = 80)]
    pub name: String,
    #[rule(required)]
    pub channels: Vec<Channel>,
    pub regions: Vec<Region>,
    #[rule(required)]
    pub product: Option<crate::stock::ProductId>,
}

#[action(public)]
pub async fn save_campaign(campaign: Valid<Campaign>) -> Result<Campaign> {
    Ok(campaign.into_inner())
}

/// Products with code, name and price columns.
#[action(permission = "stock.product.read")]
pub async fn find_product_rows(
    q: LookupQuery,
    store: crate::stock::ProductStore,
    ctx: Ctx,
) -> Result<Page<LookupItem>> {
    let items = store
        .all()?
        .into_iter()
        .map(|p| {
            LookupItem::new(p.id.0, p.name.clone()).cells([
                p.code,
                p.name,
                p.price.format(ctx.locale()),
            ])
        })
        .collect();
    Ok(q.apply(items))
}

#[screen(route = "/choices", permission = "stock.product.read")]
pub fn input_choices() -> Screen {
    let campaign = Campaign {
        name: "Spring tea week".into(),
        channels: vec![Channel::Email, Channel::Sms],
        regions: vec![Region::Marmara, Region::Aegean],
        ..Campaign::default()
    };
    Screen::new(t!("inputs.choices")).body(
        Panel::new(t!("inputs.panel_campaign")).add(
            Form::of(campaign)
                .field(Campaign::name)
                .row(|r| {
                    r.field(Campaign::channels)
                        .field(Campaign::regions)
                })
                .lookup_columns(
                    Campaign::product,
                    find_product_rows,
                    [
                        t!("inputs.col_code"),
                        t!("inputs.col_name"),
                        t!("inputs.col_price"),
                    ],
                )
                .submit(t!("inputs.save_campaign"), save_campaign)
                .on_success(|_, ui| ui.toast(t!("inputs.campaign_saved"))),
        ),
    )
}

/// Recounts stock slowly, so the busy overlay shows.
#[action(permission = "stock.product.read")]
pub async fn recount_stock(_: (), store: crate::stock::ProductStore) -> Result<usize> {
    tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
    Ok(store.all()?.len())
}

/// Counts products at once; the busy overlay never shows for it.
#[action(permission = "stock.product.read")]
pub async fn count_stock(_: (), store: crate::stock::ProductStore) -> Result<usize> {
    Ok(store.all()?.len())
}

/// A shipment note, with help on its fields.
#[model]
pub struct ShipmentNote {
    #[rule(required, max_len = 20)]
    pub reference: String,
    #[rule(max_len = 200)]
    pub instructions: Option<String>,
}

#[action(public)]
pub async fn save_shipment_note(note: Valid<ShipmentNote>) -> Result<ShipmentNote> {
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    Ok(note.into_inner())
}

#[screen(route = "/overlays", permission = "stock.product.read")]
pub fn input_overlays() -> Screen {
    let counted = |n: &Result<usize>, ui: &mut UiCtx| {
        if let Ok(n) = n {
            let n = *n;
            ui.toast(t!("inputs.counted", n = n));
        }
    };
    Screen::new(t!("inputs.overlays"))
        .toolbar(|t| {
            t.button(t!("inputs.new_note"))
                .icon(Icon::Plus)
                .tip(t!("inputs.new_note_tip"))
        })
        .body(
            Stack::new()
                .add(
                    Panel::new(t!("inputs.panel_actions")).add(
                        Row::new()
                            .compact()
                            .col(
                                ActionButton::new(t!("inputs.count"), count_stock, ())
                                    .tip(t!("inputs.count_tip"))
                                    .on_success(counted),
                            )
                            .col(
                                Busy::new().add(
                                    ActionButton::new(t!("inputs.recount"), recount_stock, ())
                                        .tip(t!("inputs.recount_tip"))
                                        .on_success(counted),
                                ),
                            )
                            .col(
                                Popover::new(t!("inputs.carrier"))
                                    .title(t!("inputs.carrier_title"))
                                    .add(Alert::info(t!("inputs.carrier_text"))),
                            ),
                    ),
                )
                .add(
                    Panel::new(t!("inputs.panel_note")).add(
                        Busy::new().add(
                            Form::of(ShipmentNote::default())
                                .field(ShipmentNote::reference)
                                .tip(ShipmentNote::reference, t!("inputs.reference_tip"))
                                .field(ShipmentNote::instructions)
                                .submit(t!("inputs.save_note"), save_shipment_note)
                                .on_success(|_, ui| ui.toast(t!("inputs.note_saved"))),
                        ),
                    ),
                ),
        )
}

/// Saves an order draft. Nothing is stored.
#[action(permission = "stock.product.read")]
pub async fn save_order_draft(code: String) -> Result<String> {
    Ok(code)
}

/// Sends an order to the supplier. Needs a permission the demo user may lack.
#[action(permission = "sales.order.send")]
pub async fn send_to_supplier(code: String) -> Result<String> {
    Ok(code)
}

/// Archives an order. Nothing is stored.
#[action(permission = "stock.product.read")]
pub async fn archive_order(code: String) -> Result<String> {
    Ok(code)
}

#[screen(route = "/commands", permission = "stock.product.read")]
pub fn input_commands() -> Screen {
    let code = "SO-2026-0142".to_string();
    Screen::new(t!("inputs.commands")).body(
        Panel::new(t!("inputs.panel_order")).add(
            Row::new()
                .compact()
                .col(
                    MenuButton::split(t!("inputs.save_draft"), save_order_draft, code.clone())
                        .action_then(
                            t!("inputs.send"),
                            send_to_supplier,
                            code.clone(),
                            |_, ui| ui.notify(Toast::success(t!("inputs.sent"))),
                        )
                        .action_then(
                            t!("inputs.save_copy"),
                            save_order_draft,
                            code.clone(),
                            |_, ui| {
                                ui.notify(Toast::info(t!("inputs.copied")).action(
                                    t!("inputs.open_fields"),
                                    Effect::navigate(input_fields, ()),
                                ))
                            },
                        ),
                )
                .col(
                    MenuButton::new(t!("inputs.more"))
                        .item(t!("inputs.open_fields"), Effect::navigate(input_fields, ()))
                        .separator()
                        .action_then(t!("inputs.archive"), archive_order, code, |_, ui| {
                            ui.notify(Toast::warning(t!("inputs.archived")))
                        })
                        .confirm(t!("inputs.archive_confirm")),
                ),
        ),
    )
}

pub fn feature() -> Feature {
    Feature::new("inputs")
        .route_prefix("/inputs")
        .guard(SignedIn)
        .screen(input_fields)
        .screen(input_choices)
        .screen(input_overlays)
        .screen(input_commands)
        .menu(|m| {
            m.group(t!("menu.inputs"), Icon::Edit)
                .item(t!("inputs.fields"), input_fields)
                .item(t!("inputs.choices"), input_choices)
                .item(t!("inputs.overlays"), input_overlays)
                .item(t!("inputs.commands"), input_commands)
        })
}
