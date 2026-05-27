//! 幻影属性探针示例。
//!
//! 直接构造一个带 `?shadow` 后缀的战斗召唤物，执行召唤物运行时初始化后
//! 打印名字系数、名字字节与八围结果。用于排查幻影/克隆继承规则是否与原版一致。

use tswn_core::engine::storage::Storage;
use tswn_core::player::Player;
use tswn_core::player::skill::act::minion::prepare_combat_minion;

fn main() {
    let storage = Storage::new_arc();
    let mut shadow = Player::new_and_init(Some("\u{0002}".to_string()), "33554466?shadow".to_string(), None, storage)
        .expect("shadow should init");
    prepare_combat_minion(&mut shadow);
    shadow.build();
    let status = shadow.get_status();
    println!("name_factor={}", shadow.get_name_factor());
    println!("name_base={:?}", &shadow.name_base[0..32]);
    println!(
        "status atk={} def={} spd={} agi={} mag={} mdf={} itl={} hp={} mp={} attr_sum={}",
        status.attack,
        status.defense,
        status.speed,
        status.agility,
        status.magic,
        status.resistance,
        status.wisdom,
        status.max_hp,
        status.magic_point,
        shadow.attr_sum()
    );
}
