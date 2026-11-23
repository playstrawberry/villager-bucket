use crate::{
    appearance, lang,
    policy::{self, Denial, Face, Hand, Interaction, Item, Plan, Refusal, Target},
};

fn capture() -> Interaction {
    Interaction {
        cancelled: false,
        permitted: true,
        connected_alive: true,
        hand: Some(Hand::Main),
        item: Item::EmptyBucket { count: 1 },
        target: Target::Villager,
        output_room: true,
    }
}

fn release() -> Interaction {
    Interaction {
        item: Item::OccupiedBucket { count: 1 },
        target: Target::Block {
            position: [1, 64, 2],
            face: Face::Up,
        },
        ..capture()
    }
}

#[test]
fn capture_uses_the_clicked_hand() {
    for hand in [Hand::Main, Hand::Off] {
        assert_eq!(
            policy::plan(Interaction {
                hand: Some(hand),
                ..capture()
            }),
            Ok(Some(Plan::Capture { hand })),
        );
    }
}

#[test]
fn release_uses_each_clicked_face() {
    for (face, destination) in [
        (Face::Down, [1, 63, 2]),
        (Face::Up, [1, 65, 2]),
        (Face::North, [1, 64, 1]),
        (Face::South, [1, 64, 3]),
        (Face::West, [0, 64, 2]),
        (Face::East, [2, 64, 2]),
    ] {
        assert_eq!(
            policy::plan(Interaction {
                target: Target::Block {
                    position: [1, 64, 2],
                    face
                },
                ..release()
            }),
            Ok(Some(Plan::Release {
                hand: Hand::Main,
                destination
            })),
        );
    }
}

#[test]
fn already_cancelled_actions_never_proceed() {
    assert_eq!(
        policy::plan(Interaction {
            cancelled: true,
            ..capture()
        }),
        Err(Denial::Cancelled)
    );
    assert_eq!(
        policy::plan(Interaction {
            cancelled: true,
            ..release()
        }),
        Err(Denial::Cancelled)
    );
}

#[test]
fn permission_missing_never_clears_trades_or_changes_either_side() {
    assert_eq!(
        policy::plan(Interaction {
            permitted: false,
            ..capture()
        }),
        Err(Denial::Permission)
    );
    assert_eq!(
        policy::plan(Interaction {
            permitted: false,
            ..release()
        }),
        Err(Denial::Permission)
    );
}

#[test]
fn dead_or_disconnected_actor_cannot_commit() {
    assert_eq!(
        policy::plan(Interaction {
            connected_alive: false,
            ..capture()
        }),
        Err(Denial::Stale)
    );
    assert_eq!(
        policy::plan(Interaction {
            connected_alive: false,
            ..release()
        }),
        Err(Denial::Stale)
    );
}

#[test]
fn full_inventory_refuses_capture_even_with_a_single_empty_bucket() {
    assert_eq!(
        policy::plan(Interaction {
            output_room: false,
            ..capture()
        }),
        Err(Denial::Full),
    );
    assert_eq!(
        policy::plan(Interaction {
            item: Item::EmptyBucket { count: 2 },
            output_room: false,
            ..capture()
        }),
        Err(Denial::Full),
    );
}

#[test]
fn missing_hand_is_not_guessed_from_the_selected_slot() {
    assert_eq!(
        policy::plan(Interaction {
            hand: None,
            ..capture()
        }),
        Err(Denial::Invalid)
    );
    assert_eq!(
        policy::plan(Interaction {
            hand: None,
            ..release()
        }),
        Err(Denial::Invalid)
    );
}

#[test]
fn invalid_stack_counts_and_stacked_captured_buckets_are_refused() {
    for count in [0, 17, 64, 255] {
        assert_eq!(
            policy::plan(Interaction {
                item: Item::EmptyBucket { count },
                ..capture()
            }),
            Err(Denial::Invalid),
        );
    }
    for count in [0, 2, 16, 255] {
        assert_eq!(
            policy::plan(Interaction {
                item: Item::OccupiedBucket { count },
                ..release()
            }),
            Err(Denial::Invalid),
        );
    }
}

#[test]
fn unrelated_entities_items_and_air_are_ignored() {
    assert_eq!(
        policy::plan(Interaction {
            target: Target::Other,
            ..capture()
        }),
        Ok(None)
    );
    assert_eq!(
        policy::plan(Interaction {
            item: Item::Other,
            ..capture()
        }),
        Ok(None)
    );
    assert_eq!(
        policy::plan(Interaction {
            target: Target::Other,
            ..release()
        }),
        Ok(None)
    );
}

#[test]
fn invalid_and_overflowing_block_positions_are_refused() {
    for position in [
        [i32::MAX, 0, 0],
        [0, i32::MAX, 0],
        [0, 0, i32::MIN],
        [30_000_000, 0, 0],
    ] {
        assert_eq!(
            policy::plan(Interaction {
                target: Target::Block {
                    position,
                    face: Face::East
                },
                ..release()
            }),
            Err(Denial::Invalid),
        );
    }
}

#[test]
fn every_refusal_the_player_should_hear_has_a_catalog_entry() {
    let told = [
        Denial::Permission,
        Denial::Full,
        Denial::Invalid,
        Denial::Obstructed,
        Denial::Protected,
        Denial::Refused(Refusal::Damaged),
        Denial::Refused(Refusal::Cancelled),
    ];
    for denial in told {
        let key = denial.message().expect("a message");
        for (locale, _) in lang::FILES {
            assert_ne!(lang::text(locale, key), key, "{locale} {key}");
        }
    }
    // Who cancelled explains it; a stale click needs no word.
    for silent in [Denial::Cancelled, Denial::Stale, Denial::Unsupported] {
        assert_eq!(silent.message(), None);
    }
}

#[test]
fn fallback_has_a_localized_name_and_two_lore_lines() {
    for (locale, _) in lang::FILES {
        let appearance = appearance::plain(locale);
        assert!(!appearance.name.is_empty());
        assert_eq!(appearance.lore.len(), 2);
        assert!(appearance.lore.iter().all(|line| !line.is_empty()));
    }
    assert_ne!(appearance::plain("en"), appearance::plain("tr"));
    assert_eq!(crate::models::NAME, "villager_bucket");
}

#[test]
fn every_catalog_has_exactly_the_same_keys_and_no_empty_values() {
    let keys = |file: &str| -> std::collections::BTreeSet<String> {
        file.lines()
            .map(|line| {
                let (key, value) = line.split_once('=').expect("catalog line");
                assert!(!value.is_empty());
                assert!(
                    !value.contains(['{', '}']),
                    "these catalogs accept no arguments"
                );
                key.to_owned()
            })
            .collect()
    };
    let english = keys(lang::FILES[0].1);
    for (locale, file) in lang::FILES {
        assert_eq!(keys(file), english, "{locale}");
        assert_eq!(
            file.lines().count(),
            english.len(),
            "duplicate key in {locale}"
        );
        for key in &english {
            assert_ne!(lang::text(locale, key), *key);
        }
    }
}

#[test]
fn permission_definition_grants_only_the_single_use_node() {
    let permission = crate::use_permission();
    assert_eq!(permission.node, "villager-bucket:use");
    assert!(matches!(
        permission.default,
        strawberry_plugin_api::permission::PermissionDefault::Allow
    ));
    assert!(permission.children.is_empty());
    assert!(!permission.description.is_empty());
}

#[test]
fn plugin_metadata_requires_only_the_shared_model_plugin() {
    use strawberry_plugin_api::Plugin;
    let metadata = crate::VillagerBucket::new().metadata();
    assert_eq!(metadata.name, "villager-bucket");
    assert_eq!(metadata.version, env!("CARGO_PKG_VERSION"));
    // Reading a stack's model id (`strawberry_tools::models::model_id`) needs no permission.
    assert!(metadata.permissions.is_empty());
    assert_eq!(metadata.dependencies, ["custom-models"]);
}

#[test]
fn unsupported_and_malformed_locales_fall_back_to_english() {
    for locale in ["", "de_de", "🦀", "中文", "e", "EN_us"] {
        assert_eq!(
            lang::text(locale, "bucket_name"),
            lang::text("en", "bucket_name")
        );
    }
    assert_eq!(
        lang::text("FR_fr", "bucket_name"),
        lang::text("fr", "bucket_name")
    );
}
