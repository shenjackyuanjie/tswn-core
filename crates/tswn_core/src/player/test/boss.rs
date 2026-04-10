use super::*;

#[test]
fn boss_has_higher_state_immunity() {
    let storage = Storage::new_arc();
    let boss = Player::new_from_namerena_raw("saitama@!".to_string(), storage.clone()).unwrap();
    let normal = Player::new_from_namerena_raw("normal".to_string(), storage.clone()).unwrap();
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
    };
    assert!(boss.check_immune("fire", &mut randomer));
    assert!(!normal.check_immune("fire", &mut randomer));
}
