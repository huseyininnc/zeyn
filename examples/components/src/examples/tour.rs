//! Tour examples: a guided tour of the page.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .gap(4)
        .add(Tour::new("Show me around"))
        .add(
            Row::new()
                .col(
                    TourStop::on(Kpi::new("Orders", 1_284.0).delta(3.1))
                        .title("Orders")
                        .body("How many orders came in this month, against the month before."),
                )
                .col(
                    TourStop::on(Kpi::new("New customers", 312.0).delta(-2.0))
                        .title("New customers")
                        .body("Customers with their first order this month."),
                ),
        )
        .add(
            TourStop::on(
                Table::new("Opening hours")
                    .head(["Day", "Opens", "Closes"])
                    .row(["Weekdays", "09:00", "18:00"])
                    .row(["Saturday", "10:00", "14:00"])
                    .row_headers(),
            )
            .title("Opening hours")
            .body("The last stop. Finish ends the tour and gives focus back to the button."),
        )
        .into()
}

/// What the catalog shows of `Tour`.
pub const ENTRY: Entry = Entry {
    name: "Tour",
    docs: "components/display#tour",
    api: &["Tour", "TourStop"],
    overview: &[ex!(overview)],
    examples: &[],
};
