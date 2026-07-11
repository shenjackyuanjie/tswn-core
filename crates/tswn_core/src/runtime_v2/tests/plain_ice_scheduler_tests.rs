use super::*;

#[test]
fn ice_pre_step_zero_step_does_not_release_at_threshold() {
    let mut states = StateStore::default();
    states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));
    let generation = states.generation();

    let (step, released) = states.apply_ice_pre_step(0, crate::player::MOVE_POINT_THRESHOLD + 1);

    assert_eq!(step, 0);
    assert!(!released);
    assert!(states.is_frozen());
    assert_eq!(states.generation(), generation);
}

#[test]
fn ice_pre_step_positive_step_releases_when_crossing_threshold() {
    let mut states = StateStore::default();
    states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));

    let (step, released) = states.apply_ice_pre_step(1, crate::player::MOVE_POINT_THRESHOLD);

    assert_eq!(step, 0);
    assert!(released);
    assert!(!states.is_frozen());
}
