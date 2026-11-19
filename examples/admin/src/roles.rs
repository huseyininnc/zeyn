//! Roles and the permissions each one grants. Screens and actions name these permissions, and the
//! server checks them on every request.

use zeyn::prelude::*;

/// What a user may do in the admin app.
#[choice]
pub enum Role {
    Admin,
    Manager,
    Viewer,
}

/// Every permission of the app, in the order the roles screen lists them.
pub const PERMISSIONS: &[&str] = &[
    "admin.dashboard.read",
    "admin.orders.read",
    "admin.orders.write",
    "admin.customers.read",
    "admin.customers.write",
    "admin.products.read",
    "admin.products.write",
    "admin.settings.read",
    "admin.settings.write",
    "admin.users.read",
    "admin.users.write",
];

impl Role {
    /// Every role, in the order the roles screen lists them.
    pub const ALL: [Role; 3] = [Role::Admin, Role::Manager, Role::Viewer];

    /// The permissions the role grants: administrators everything, managers the daily work
    /// without users and settings changes, viewers reading the sales data only.
    pub fn permissions(self) -> &'static [&'static str] {
        match self {
            Role::Admin => PERMISSIONS,
            Role::Manager => &[
                "admin.dashboard.read",
                "admin.orders.read",
                "admin.orders.write",
                "admin.customers.read",
                "admin.customers.write",
                "admin.products.read",
                "admin.products.write",
                "admin.settings.read",
                "admin.users.read",
            ],
            Role::Viewer => &[
                "admin.dashboard.read",
                "admin.orders.read",
                "admin.customers.read",
                "admin.products.read",
            ],
        }
    }

    /// The role named in configuration (`Admin`, `Manager` or `Viewer`, any case).
    pub fn named(name: &str) -> Option<Role> {
        Role::ALL
            .into_iter()
            .find(|r| format!("{r:?}").eq_ignore_ascii_case(name))
    }

    /// A signed in user with this role's permissions.
    pub fn identity(self, user: &str) -> Identity {
        Identity::user(user).with_permissions(self.permissions().iter().copied())
    }
}
