#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub name: String,
    pub lore: Vec<String>,
}

/// Vanilla-only fallback. Model items and their translated names belong solely
/// to custom-models and are requested through the shared IPC client.
pub fn plain(locale: &str) -> Appearance {
    Appearance {
        name: crate::lang::text(locale, "bucket_name"),
        lore: vec![
            crate::lang::text(locale, "bucket_lore"),
            crate::lang::text(locale, "bucket_state"),
        ],
    }
}

/// Decoration only, not a valid occupied bucket or a proof of captured state.
/// The adapter must attach/validate the complete snapshot and enforce
/// maximum stack size one. This function never grants an item.
pub fn vanilla_stack(locale: &str) -> strawberry_plugin_api::ItemStack {
    use strawberry_plugin_api::text::TextComponent;
    let stack = strawberry_plugin_api::ItemStack::new("minecraft:bucket", 1);
    let appearance = plain(locale);
    stack.set_custom_name(Some(TextComponent::text(&appearance.name)));
    stack.set_lore(
        appearance
            .lore
            .iter()
            .map(|line| TextComponent::text(line))
            .collect(),
    );
    stack
}
