//! Villager Bucket: carry a villager in a bucket through the engine's complete entity
//! snapshot, its checked removal and its checked restore.

#![forbid(unsafe_code)]

pub mod appearance;
pub mod exchange;
pub mod host;
pub mod lang;
pub mod models;
pub mod policy;
pub mod snapshot;

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
    context: &Context,
    register: impl FnOnce(
        &custom_models_api::Model<'_>,
    ) -> std::result::Result<
        custom_models_api::Registered,
        custom_models_api::Error,
    >,
) -> Result<()> {
    let models = models::Models::connect_with("villager-bucket", register);
    context.register_permission(&use_permission())?;
    host::subscribe(context, &std::sync::Arc::new(host::Shared { models }))
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
            // None: `custom_models_api::model_id` only reads a stack's model id
            // (`content.item-id`), which needs no permission; custom-models writes the pack.
            permissions: Vec::new(),
        }
    }

    fn on_load(&self, context: Context) -> Result<()> {
        initialize(&context, custom_models_api::register)
    }
}

register_plugin!(VillagerBucket);

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transfer_tests;
