use super::models::Customer;
use super::store::CustomerStore;
use zeyn::prelude::*;

/// Customers for lists and the API.
#[action(permission = "customers.customer.read")]
pub async fn search_customers(q: DataQuery, store: CustomerStore) -> Result<Page<Customer>> {
    Ok(q.apply(store.all()?))
}

/// Creates or updates a customer.
#[action(permission = "customers.customer.write", audit)]
pub async fn save_customer(customer: Valid<Customer>, store: CustomerStore) -> Result<Customer> {
    store.save(customer.into_inner())
}
