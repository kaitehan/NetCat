use netcat_lib::{
    pet::{
        load_pet_preferences, save_pet_placement, set_pet_enabled, PetEdge, PetPlacement,
        PetPreferences,
    },
    storage::Store,
};

fn store() -> Store {
    Store::open_in_memory(100, 1024).unwrap()
}

#[test]
fn defaults_to_an_enabled_pet_without_a_saved_placement() {
    assert_eq!(
        load_pet_preferences(&store()).unwrap(),
        PetPreferences {
            version: 1,
            enabled: true,
            placement: None,
        }
    );
}

#[test]
fn persists_visibility_and_a_valid_relative_placement() {
    let store = store();
    assert!(!set_pet_enabled(&store, false).unwrap().enabled);

    let placement = PetPlacement {
        monitor_name: Some("Studio Display".into()),
        edge: PetEdge::Left,
        offset: 0.375,
    };
    let saved = save_pet_placement(&store, placement.clone()).unwrap();

    assert!(!saved.enabled);
    assert_eq!(saved.placement, Some(placement));
    assert_eq!(load_pet_preferences(&store).unwrap(), saved);
}

#[test]
fn malformed_or_unsupported_saved_preferences_fall_back_to_defaults() {
    let store = store();
    for value in [
        "not-json",
        r#"{"version":2,"enabled":false,"placement":null}"#,
        r#"{"version":1,"enabled":true,"placement":{"monitorName":null,"edge":"right","offset":9}}"#,
    ] {
        store.set_setting("desktop_pet_preferences", value).unwrap();
        assert_eq!(
            load_pet_preferences(&store).unwrap(),
            PetPreferences::default()
        );
    }
}

#[test]
fn rejects_non_finite_or_out_of_range_command_placements() {
    let store = store();
    for offset in [f64::NAN, -0.1, 1.1] {
        let error = save_pet_placement(
            &store,
            PetPlacement {
                monitor_name: None,
                edge: PetEdge::Bottom,
                offset,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "PET_PLACEMENT_INVALID");
    }
}
