use zeyn::prelude::*;

/// A customer shared by every app of the organization.
#[entity(table = "customers")]
pub struct Customer {
    #[key]
    pub id: CustomerId,
    #[rule(required, max_len = 20, unique)]
    pub code: String,
    #[rule(required, max_len = 200)]
    pub name: String,
    #[rule(email)]
    pub email: Option<String>,
    pub segment: Segment,
    pub active: bool,
    #[version]
    pub version: i64,
}

/// Customer segment.
#[choice]
pub enum Segment {
    Retail,
    Corporate,
    Public,
}
