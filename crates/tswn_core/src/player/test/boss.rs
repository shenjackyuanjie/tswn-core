//! Boss 专属行为测试。
//!
//! 覆盖普通玩家与 Boss 在免疫、状态处理和专属规则上的差异，
//! 避免 Boss 标记只影响名字解析却没有进入运行时判定。

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
