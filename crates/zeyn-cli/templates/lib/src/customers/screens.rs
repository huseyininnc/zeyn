use super::actions::{save_customer, search_customers};
use super::models::{Customer, CustomerId};
use super::store::CustomerStore;
use zeyn::prelude::*;

/// Which customer the editor opens; `None` creates one.
#[params]
pub struct CustomerParams {
    pub id: Option<CustomerId>,
}

/// Customer list.
#[screen(route = "/", permission = "customers.customer.read")]
pub fn customer_list() -> Screen {
    Screen::new(t!("customers.list"))
        .toolbar(|t| t.button(t!("customers.new")).icon(Icon::Plus).opens(customer_editor, CustomerParams { id: None }))
        .body(
            Grid::auto(search_customers)
                .mode(GridMode::Server)
                .on_open(|c| Window::open(customer_editor, CustomerParams { id: Some(c.id) })),
        )
}

/// Customer editor window.
#[screen(permission = "customers.customer.write")]
pub fn customer_editor(params: CustomerParams, store: CustomerStore) -> Result<Screen> {
    let customer = match params.id {
        Some(id) => store.get(id)?,
        None => Customer { active: true, ..Customer::default() },
    };
    Ok(Screen::window(t!("customers.editor")).body(
        Form::auto(customer).submit(t!("customers.save"), save_customer).on_success(|_, ui| {
            ui.toast(t!("customers.saved"));
            ui.close_and_refresh();
        }),
    ))
}
