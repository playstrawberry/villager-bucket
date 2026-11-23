use std::cell::Cell;

use strawberry_tools::models::{Error, ErrorKind, Registered};

use crate::models::{Delivery, Models, Place, VisualId};

const OWNER: &str = "villager-bucket";
const ID: &str = "villager-bucket.villager_bucket";

fn connected(ready: bool) -> Models {
    Models::connect_with(OWNER, |_| {
        Ok(Registered {
            id: ID.into(),
            ready,
        })
    })
}

fn error(kind: ErrorKind) -> Error {
    Error {
        kind,
        detail: String::new(),
    }
}

#[test]
fn installed_plugin_registers_its_model_instead_of_using_a_platform_builtin() {
    let calls = Cell::new(0);
    let models = Models::connect_with(OWNER, |model| {
        calls.set(calls.get() + 1);
        assert_eq!(model.name, "villager_bucket");
        assert_eq!(model.base, Some("minecraft:paper"));
        assert_eq!(model.stack, Some(1));
        assert_eq!(model.files.len(), 1);
        assert_eq!(model.files[0].0, "texture.png");
        assert!(model.files[0].1.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(model.names.len(), 4);
        for (locale, text) in model.names {
            assert_eq!(*text, crate::lang::text(locale, "bucket_name"));
        }
        let request = strawberry_tools::models::model_request(model).unwrap();
        assert!(request.starts_with(b"model villager_bucket\n"));
        assert!(!request.windows(4).any(|part| part == b"use "));
        Ok(Registered {
            id: ID.into(),
            ready: false,
        })
    });
    assert_eq!(calls.get(), 1);
    assert!(models.registered());
    assert!(!models.ready());
    assert_eq!(crate::models::PLUGIN, "custom-models");
}

#[test]
fn pending_content_selects_vanilla_on_the_services_explicit_unavailable_answer() {
    let models = connected(false);
    assert!(models.registered());
    assert_eq!(
        models.give_with(|_| Err(error(ErrorKind::Unavailable))),
        Ok(Delivery::Vanilla)
    );
    assert_eq!(
        models.hand_with(false, |_, _| Err(error(ErrorKind::Unavailable))),
        Ok(Delivery::Vanilla)
    );
}

#[test]
fn pending_registration_does_not_latch_cosmetics_off_until_restart() {
    let models = connected(false);
    let calls = Cell::new(0);
    assert_eq!(
        models.give_with(|id| {
            calls.set(calls.get() + 1);
            assert_eq!(id, ID);
            Ok(0)
        }),
        Ok(Delivery::Model(Place::Slot(0)))
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn registration_errors_and_wrong_owner_never_enable_delivery() {
    for kind in [
        ErrorKind::NotInstalled,
        ErrorKind::Unavailable,
        ErrorKind::Invalid,
        ErrorKind::Unknown,
        ErrorKind::Limit,
        ErrorKind::Full,
        ErrorKind::Other,
    ] {
        let models = Models::connect_with(OWNER, |_| Err(error(kind)));
        assert!(!models.registered());
        assert!(!models.ready());
        assert_eq!(
            models.give_with(|_| panic!("unexpected grant")),
            Ok(Delivery::Vanilla)
        );
    }
    let models = Models::connect_with(OWNER, |_| {
        Ok(Registered {
            id: "other.villager_bucket".into(),
            ready: true,
        })
    });
    assert!(!models.registered());
    assert!(!models.ready());
}

#[test]
fn give_uses_only_the_registered_id_and_returns_snapshot_attachment_slot() {
    for slot in [0, 8, 35, 40] {
        assert_eq!(
            connected(true).give_with(|id| {
                assert_eq!(id, ID);
                Ok(slot)
            }),
            Ok(Delivery::Model(Place::Slot(slot as u8)))
        );
    }
}

#[test]
fn both_hands_use_the_shared_model_without_renaming_it() {
    for off in [false, true] {
        assert_eq!(
            connected(true).hand_with(off, |id, actual| {
                assert_eq!(id, ID);
                assert_eq!(actual, off);
                Ok(())
            }),
            Ok(Delivery::Model(if off { Place::Off } else { Place::Main }))
        );
    }
}

#[test]
fn explicit_unavailable_or_not_installed_uses_vanilla_once() {
    for kind in [ErrorKind::NotInstalled, ErrorKind::Unavailable] {
        let calls = Cell::new(0);
        let fail = || {
            calls.set(calls.get() + 1);
            error(kind)
        };
        assert_eq!(
            connected(true).give_with(|_| Err(fail())),
            Ok(Delivery::Vanilla)
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(
            connected(true).hand_with(true, |_, _| Err(fail())),
            Ok(Delivery::Vanilla)
        );
        assert_eq!(calls.get(), 2);
    }
}

#[test]
fn full_invalid_unknown_limit_or_ambiguous_mutation_never_retries() {
    for kind in [
        ErrorKind::Full,
        ErrorKind::Invalid,
        ErrorKind::Unknown,
        ErrorKind::Limit,
        ErrorKind::Other,
    ] {
        let calls = Cell::new(0);
        let fail = || {
            calls.set(calls.get() + 1);
            error(kind)
        };
        assert_eq!(connected(true).give_with(|_| Err(fail())), Err(kind));
        assert_eq!(calls.get(), 1);
        assert_eq!(
            connected(true).hand_with(false, |_, _| Err(fail())),
            Err(kind)
        );
        assert_eq!(calls.get(), 2);
    }
}

#[test]
fn out_of_range_mutation_result_is_not_used_or_retried() {
    for slot in [41, 255, u32::MAX] {
        let calls = Cell::new(0);
        assert_eq!(
            connected(true).give_with(|_| {
                calls.set(calls.get() + 1);
                Ok(slot)
            }),
            Err(ErrorKind::Other)
        );
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn cosmetic_identity_is_read_without_ipc_even_while_pack_is_pending() {
    for ready in [false, true] {
        let models = connected(ready);
        assert_eq!(models.identify_id(Some(ID)), VisualId::VillagerBucket);
        for id in [
            None,
            Some(""),
            Some("villager_bucket"),
            Some("other.villager_bucket"),
            Some("villager-bucket.villager_bucket\n"),
        ] {
            assert_eq!(models.identify_id(id), VisualId::Other);
        }
    }
    // Recognition yields no snapshot bytes or host authority to spawn anything.
    let _only_cosmetic: VisualId = connected(false).identify_id(Some(ID));
}
