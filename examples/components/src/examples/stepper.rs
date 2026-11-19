//! Stepper examples: where a process stands, across or down the page.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Stepper::new("Order progress")
        .step("Placed")
        .step("Packed")
        .step("Shipped")
        .description("Left the warehouse on Monday")
        .step("Delivered")
        .current(2)
        .into()
}

pub fn vertical(_ctx: &Ctx) -> Node {
    Stepper::new("Account setup")
        .step("Create the account")
        .description("Email and password")
        .step("Verify the email")
        .description("We sent a link to your address")
        .step("Invite the team")
        .description("Add the people you work with")
        .current(1)
        .vertical()
        .into()
}

pub fn finished(_ctx: &Ctx) -> Node {
    Stepper::new("Import")
        .step("Upload")
        .step("Check")
        .step("Save")
        .finished()
        .into()
}

/// What the catalog shows of `Stepper`.
pub const ENTRY: Entry = Entry {
    name: "Stepper",
    docs: "components/content#stepper",
    api: &["Stepper"],
    overview: &[ex!(overview)],
    examples: &[ex!(vertical), ex!(finished)],
};
