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
                .row(|r| r.field(SupplierTerms::phone).field(SupplierTerms::iban))
                .field(SupplierTerms::portal_password)
                .control(SupplierTerms::portal_password, Control::PasswordMeter)
                .submit(t!("inputs.save"), save_terms)
                .on_success(|_, ui| ui.toast(t!("inputs.saved"))),
        ),
    )
}

pub fn feature() -> Feature {
    Feature::new("inputs")
        .route_prefix("/inputs")
        .guard(SignedIn)
        .screen(input_fields)
        .menu(|m| m.group(t!("menu.inputs"), Icon::Edit).item(t!("inputs.fields"), input_fields))
}
