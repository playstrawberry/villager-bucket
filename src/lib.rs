//! Villager Bucket. The binding awaits complete snapshots and checked removal.

#![forbid(unsafe_code)]

pub mod appearance;
pub mod exchange;
pub mod host;
pub mod lang;
pub mod models;
pub mod policy;

pub const USE_PERMISSION: &str = "villager-bucket:use";

use strawberry_plugin_api::{
    Context, Plugin, PluginMetadata, Result,
    permission::{Permission, PermissionDefault},
    register_plugin,
};

pub fn use_permission() -> Permission {
    Permission {
        node: USE_PERMISSION.into(),
        description: lang::text("en", "permission"),
        default: PermissionDefault::Allow,
        children: Vec::new(),
    }
}

struct VillagerBucket;

fn initialize(
    register: impl FnOnce(
        &custom_models_api::Model<'_>,
    ) -> std::result::Result<
        custom_models_api::Registered,
        custom_models_api::Error,
    >,
) -> Result<()> {
    let _models = models::Models::connect_with("villager-bucket", register);
    host::require_support().map_err(|_| lang::text("en", "blocked"))
}

impl Plugin for VillagerBucket {
    fn new() -> Self {
        Self
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "villager-bucket".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            authors: vec!["Strawberry Interactive LLC".into()],
            description: lang::text("en", "description"),
            dependencies: vec![models::PLUGIN.into()],
            permissions: Vec::new(),
        }
    }

    fn on_load(&self, _context: Context) -> Result<()> {
        // Bind the adapter when complete snapshots and checked removal are
        // supported. A partial villager copy would discard saved properties.
        initialize(custom_models_api::register)
    }
}

register_plugin!(VillagerBucket);

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transfer_tests;
