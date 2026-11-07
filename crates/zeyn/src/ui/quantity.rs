//! [`QuantityStepper`]: a number with minus and plus buttons whose changes an action checks.

use super::layout::into_node;
use super::{DataSource, dom_id};
use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx};
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::marker::PhantomData;

into_node!(QuantityStepper);

/// The largest quantity a stepper accepts, whatever its range says.
pub const MAX_QUANTITY: i64 = 9_999;

/// What the action of a [`QuantityStepper`] gets: the item (set by the screen, never by the
/// browser) and the quantity the user asked for, already inside the stepper's range.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantityChange {
    pub target: String,
    pub quantity: i64,
}

/// What the action of a [`QuantityStepper`] answers: the quantity it kept, and the most the
/// user may have now, such as the units in stock. The stepper shows the kept quantity, and when
/// it is less than the user asked for, says how many are available.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantityState {
    pub quantity: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
}

impl QuantityState {
    /// The kept quantity.
    pub fn new(quantity: i64) -> Self {
        QuantityState {
            quantity,
            max: None,
        }
    }

    /// The most the user may have now, such as the units in stock.
    pub fn max(mut self, max: i64) -> Self {
        self.max = Some(max);
        self
    }
}

#[derive(Deserialize)]
struct ClientChange {
    quantity: i64,
}

/// Runs the action with the screen's target and the asked quantity clamped to the range.
struct QuantitySource<A> {
    target: String,
    range: (i64, i64),
    _action: PhantomData<fn() -> A>,
}

impl<A> DataSource for QuantitySource<A>
where
    A: Action<Input = QuantityChange, Output = Result<QuantityState>>,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let change = serde_json::from_value::<ClientChange>(input)
            .ok()
            .map(|c| QuantityChange {
                target: self.target.clone(),
                quantity: c
                    .quantity
                    .clamp(self.range.0, self.range.1),
            });
        Box::pin(async move {
            let change = change.ok_or(Error::NotFound)?;
            let state = A::call(ctx.clone(), change).await??;
            crate::privacy::client_json(&state, &ctx).map_err(|e| Error::Internal(e.to_string()))
        })
    }
}

/// A quantity with minus and plus buttons and a number field between them, such as the amount of
/// an item in a cart. Every change (a button press or a typed number) is sent once the user stops
/// for 400 ms, through `POST /_zeyn/data` with the CSRF check and the write rate limit, as the
/// user, with the action's permission checked on the server. The server clamps the quantity to
/// the stepper's range before the action runs; the action checks what only it knows, such as the
/// stock, and answers with the quantity it kept ([`QuantityState`]). When that is less than the
/// user asked for, the stepper shows it and says how many are available in a live region. A newer
/// change cancels the one still on its way.
///
/// The item (`target`) is set by the screen and never read from the browser. The buttons are at
/// least 24 by 24 pixels (WCAG 2.5.8), named "Decrease" and "Increase" for screen readers, and
/// turn off at the ends of the range; the field takes digits from the keyboard and arrow keys.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::{QuantityChange, QuantityState, QuantityStepper};
///
/// #[action(permission = "cart.edit")]
/// pub async fn set_quantity(change: QuantityChange) -> Result<QuantityState> {
///     let stock = 7;
///     Ok(QuantityState::new(change.quantity.min(stock)).max(stock))
/// }
///
/// let stepper = QuantityStepper::of(set_quantity, "sku-42")
///     .label("Quantity")
///     .range(1, 10)
///     .value(2);
/// ```
pub struct QuantityStepper {
    label: Option<Text>,
    target: String,
    range: (i64, i64),
    value: i64,
    source: Box<dyn DataSource>,
    make: fn(String, (i64, i64)) -> Box<dyn DataSource>,
    action: ActionMeta,
}

fn make_source<A>(target: String, range: (i64, i64)) -> Box<dyn DataSource>
where
    A: Action<Input = QuantityChange, Output = Result<QuantityState>>,
{
    Box::new(QuantitySource::<A> {
        target,
        range,
        _action: PhantomData,
    })
}

impl QuantityStepper {
    /// A stepper for `target`, an id of the item the quantity is of, whose changes `action` checks
    /// and stores. The range is 1 to 99 unless set.
    pub fn of<A>(_action: A, target: impl Into<String>) -> Self
    where
        A: Action<Input = QuantityChange, Output = Result<QuantityState>>,
    {
        let target = target.into();
        QuantityStepper {
            label: None,
            source: make_source::<A>(target.clone(), (1, 99)),
            target,
            range: (1, 99),
            value: 1,
            make: make_source::<A>,
            action: A::META,
        }
    }

    /// The smallest and the largest quantity, both kept between 0 and [`MAX_QUANTITY`]. The server
    /// clamps every change to them before the action runs.
    pub fn range(mut self, min: i64, max: i64) -> Self {
        let lo = min.clamp(0, MAX_QUANTITY);
        let hi = max.clamp(0, MAX_QUANTITY);
        self.range = if lo <= hi { (lo, hi) } else { (hi, lo) };
        self.value = self
            .value
            .clamp(self.range.0, self.range.1);
        self.source = (self.make)(self.target.clone(), self.range);
        self
    }

    /// The quantity shown first, clamped to the range.
    pub fn value(mut self, quantity: i64) -> Self {
        self.value = quantity.clamp(self.range.0, self.range.1);
        self
    }

    /// The name of the field, "Quantity" unless set.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Component for QuantityStepper {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "qty");
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.shop.quantity.{key}")));
        let label = match &self.label {
            Some(l) => r.text(l),
            None => t("label"),
        };
        let (min, max) = self.range;
        let v = self.value;
        let off = |at: bool| if at { " disabled" } else { "" };
        out.push_str(&format!(
            r#"<div class="z-qty" role="group" aria-labelledby="{id}-label" data-zeyn-quantity data-text-limit="{}" data-text-failed="{}"><label class="z-qty-label" id="{id}-label" for="{id}">{label}</label><div class="z-qty-box"><button type="button" class="z-qty-step" data-step="-1" aria-label="{}" aria-controls="{id}"{}><span aria-hidden="true">&minus;</span></button><input type="number" class="z-qty-input" id="{id}" inputmode="numeric" min="{min}" max="{max}" step="1" value="{v}" data-testid="{}.quantity.{}"><button type="button" class="z-qty-step" data-step="1" aria-label="{}" aria-controls="{id}"{}><span aria-hidden="true">+</span></button></div><span class="z-qty-status" role="status"></span></div>"#,
            t("limit"),
            t("failed"),
            t("decrease"),
            off(v <= min),
            r.scope,
            escape(&self.target),
            t("increase"),
            off(v >= max),
        ));
    }

    fn schema(&self) -> Json {
        json!({
            "type": "quantity_stepper",
            "action": self.action,
            "target": self.target,
            "min": self.range.0,
            "max": self.range.1,
            "value": self.value,
        })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        (key == "quantity").then_some(self.source.as_ref())
    }
}
