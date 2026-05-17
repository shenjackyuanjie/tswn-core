use tswn_core::engine::storage::Storage;
use tswn_core::player::skill::act::minion::prepare_combat_minion;
use tswn_core::player::Player;

fn main() {
    let storage = Storage::new_arc();
    let mut shadow = Player::new_and_init(
        Some("\u{0002}".to_string()),
        "33554466?shadow".to_string(),
        None,
        storage,
    )
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
