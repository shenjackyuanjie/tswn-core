//! 武器与属性派生测试。
//!
//! 覆盖武器加成、升级继承、名字字节规则和 `build()` 之后的派生属性变化。

use super::*;

#[test]
fn upgrade_uses_other_raw_name_base_rules() {
    let storage = Storage::new_arc();
    let mut lhs = Player::new_from_namerena_raw("lhs".to_string(), storage.clone()).unwrap();
    let mut rhs = Player::new_from_namerena_raw("rhs".to_string(), storage.clone()).unwrap();

    lhs.name_base = vec![0; 128];
    rhs.name_base = vec![0; 128];
    lhs.raw_name_base = [0; 128];
    rhs.raw_name_base = [0; 128];

    lhs.raw_name_base[10] = 42;
    lhs.name_base[10] = 50;
    rhs.raw_name_base[9] = 42;
    rhs.raw_name_base[10] = 99;
    rhs.name_base[10] = 1;

    lhs.raw_name_base[8] = 77;
    lhs.name_base[8] = 10;
    rhs.raw_name_base[6] = 77;
    rhs.raw_name_base[8] = 88;
    rhs.name_base[8] = 2;

    lhs.upgrade(&rhs);

    assert_eq!(lhs.name_base[10], 99);
    assert_eq!(lhs.name_base[8], 88);
}

#[test]
fn build_applies_s11_weapon_bonus() {
    let storage = Storage::new_arc();
    let mut base = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let mut with_weapon = Player::new_from_namerena_raw("aaa+剁手刀".to_string(), storage.clone()).unwrap();
    base.build();
    with_weapon.build();
    assert_eq!(with_weapon.attr[0], base.attr[0] + 11);
    assert_eq!(with_weapon.attr[2], base.attr[2] + 11);
}
