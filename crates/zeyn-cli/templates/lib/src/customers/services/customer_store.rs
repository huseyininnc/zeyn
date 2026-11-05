use super::super::models::{Customer, CustomerId};
use std::sync::{Arc, RwLock};
use zeyn::prelude::*;

/// Customers kept in memory. Replace it with a database backed service.
#[derive(Clone, Default)]
pub struct CustomerStore(Arc<RwLock<Vec<Customer>>>);

impl Service for CustomerStore {}

fn poisoned<T>(_: T) -> Error {
    Error::Internal("customer store lock is poisoned".into())
}

impl CustomerStore {
    pub fn all(&self) -> Result<Vec<Customer>> {
        Ok(self.0.read().map_err(poisoned)?.clone())
    }

    pub fn get(&self, id: CustomerId) -> Result<Customer> {
        self.0
            .read()
            .map_err(poisoned)?
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or(Error::NotFound)
    }

    /// Inserts (id 0) or updates; a stale row version fails with `Conflict`.
    pub fn save(&self, mut customer: Customer) -> Result<Customer> {
        let mut rows = self.0.write().map_err(poisoned)?;
        if customer.id.0 == 0 {
            customer.id = CustomerId(
                rows.iter()
                    .map(|c| c.id.0)
                    .max()
                    .unwrap_or(0)
                    + 1,
            );
            customer.version = 1;
            rows.push(customer.clone());
            return Ok(customer);
        }
        let current = rows
            .iter_mut()
            .find(|c| c.id == customer.id)
            .ok_or(Error::NotFound)?;
        if current.version != customer.version {
            return Err(Error::Conflict);
        }
        customer.version += 1;
        *current = customer.clone();
        Ok(customer)
    }
}
