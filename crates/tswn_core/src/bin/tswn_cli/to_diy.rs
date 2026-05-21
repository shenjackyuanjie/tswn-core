//! `to-diy` 子命令：将名字转换为 DIY/OL overlay 格式。

use tswn_core::engine::storage::Storage;
use tswn_core::player::Player;

pub fn run(raw: &str) {
    let storage = Storage::new_arc();
    let mut player = match Player::new_from_namerena_raw(raw.to_string(), storage) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("构建玩家失败: {err}");
            std::process::exit(1);
        }
    };
    player.build();

    println!("=== 紧凑 DIY 格式 (diy[...]) ===");
    println!("{}", player.to_diy_compact());
    println!();
    println!("=== JSON 对象格式 (ol:{{...}}) ===");
    println!("{}", player.to_ol_json());
    println!();
    println!("=== 原始信息 ===");
    println!("名字: {}", player.id_name());
    println!("队伍: {}", player.clan_name());
    let status = player.get_status();
    println!(
        "八围 (计算后): atk={} def={} spd={} agi={} mag={} res={} wis={} maxhp={}",
        status.attack,
        status.defense,
        status.speed,
        status.agility,
        status.magic,
        status.resistance,
        status.wisdom,
        status.max_hp,
    );
    println!("name_factor: {:.6}", player.get_name_factor());
}
