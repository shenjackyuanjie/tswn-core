use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};

use tswn_core::Runner;
use tswn_core::engine::update::{RunUpdate, UpdateType};
use tswn_core::player::icon::icon_from_name;

fn print_usage() {
    println!("用法:");
    println!("  namerena_cli [选项]");
    println!();
    println!("选项:");
    println!("  --raw <字符串>    使用提供的原始字符串作为输入");
    println!("  --file <文件路径>  从文件读取输入");
    println!("  --icon <名字>...  输出玩家图标信息 (可指定多个名字)");
    println!("  --help, -h        显示此帮助信息");
    println!();
    println!("  namerena_cli --raw \"a\\nb\\n\\nc\\nd\"");
    println!("  namerena_cli --file input.txt");
    println!("  namerena_cli --icon mario aaa test");
    println!("  echo \"a\\nb\\n\\nc\\nd\" | namerena_cli");
}

fn read_raw_input() -> Result<String, String> {
    let args = std::env::args().skip(1).collect::<Vec<String>>();
    if args.is_empty() {
        let mut raw = String::new();
        io::stdin().read_to_string(&mut raw).map_err(|e| format!("读取 stdin 失败: {e}"))?;
        if raw.trim().is_empty() {
            return Err("未提供 raw_namerena 输入".to_string());
        }
        return Ok(raw);
    }

    match args[0].as_str() {
        "--help" | "-h" => {
            print_usage();
            std::process::exit(0);
        }
        "--icon" => {
            if args.len() < 2 {
                eprintln!("--icon 需要至少一个名字参数");
                std::process::exit(2);
            }
            for name in &args[1..] {
                print_icon(name);
            }
            std::process::exit(0);
        }
        "--raw" => {
            if args.len() < 2 {
                return Err("--raw 需要一个字符串参数".to_string());
            }
            Ok(args[1..].join(" ").replace("\\n", "\n"))
        }
        "--file" => {
            if args.len() != 2 {
                return Err("--file 需要一个文件路径参数".to_string());
            }
            fs::read_to_string(&args[1]).map_err(|e| format!("读取文件失败: {e}"))
        }
        _ => Ok(args.join(" ").replace("\\n", "\n")),
    }
}

/// Print a TUI representation of the icon for a given player name.
fn print_icon(name: &str) {
    let icon = icon_from_name(name);
    let [br, bg, bb] = icon.bg_color;

    println!("=== Icon: {name} ===");
    println!("边框样式: {}", icon.border_style);
    println!("形状: {:?}", icon.shapes);
    println!("背景色: #{:02X}{:02X}{:02X} (索引 {})", br, bg, bb, icon.bg_color_idx);

    // TUI: render a colored block preview using ANSI true-color escape codes
    // Top border
    let border_char = match icon.border_style {
        0 => '─',
        1 => '━',
        2 => '═',
        3 => '┄',
        4 => '┅',
        5 => '╌',
        6 => '╍',
        _ => '─',
    };
    let border_line: String = std::iter::repeat_n(border_char, 18).collect();
    println!("┌{}┐", border_line);

    // Render 8 rows for the icon
    for row in 0..8 {
        print!("│");
        // Background fill with foreground shape blocks interleaved
        for col in 0..9 {
            // Determine which shape/color occupies this cell
            let shape_idx = (row * 9 + col) % (icon.shapes.len() + 1);
            if shape_idx == 0 {
                // Background cell
                print!("\x1b[48;2;{br};{bg};{bb}m  \x1b[0m");
            } else {
                let ci = (shape_idx - 1) % icon.fg_colors.len();
                let [fr, fg, fb] = icon.fg_colors[ci];
                print!("\x1b[48;2;{fr};{fg};{fb}m  \x1b[0m");
            }
        }
        println!("│");
    }
    println!("└{}┘", border_line);

    // Foreground colors detail
    for (i, (idx, color)) in icon.fg_color_indices.iter().zip(icon.fg_colors.iter()).enumerate() {
        let [r, g, b] = *color;
        println!(
            "前景色 {i}: \x1b[48;2;{r};{g};{b}m    \x1b[0m #{r:02X}{g:02X}{b:02X} (索引 {idx}, 形状 {})",
            icon.shapes[i]
        );
    }
    println!();
}

fn plr_name(runner: &Runner, id: usize) -> String {
    runner
        .storage
        .get_player(&id)
        .map(|plr| plr.display_name())
        .unwrap_or_else(|| format!("#{id}"))
}

fn fmt_update(runner: &Runner, update: &RunUpdate) -> String {
    let caster = plr_name(runner, update.caster);
    let target = plr_name(runner, update.target);
    let targets = if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update.targets.iter().map(|id| plr_name(runner, *id)).collect::<Vec<String>>().join(",")
    };

    let mut msg = update.message.clone();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    msg = msg.replace("[2]", &targets);
    if update.score > 0 {
        format!("{msg}  (+{})", update.score)
    } else {
        msg
    }
}

fn print_all_players(runner: &Runner) {
    println!("=== 玩家状态 ===");
    let player_ids = runner.storage.all_player_ids();
    for id in player_ids {
        if let Some(plr) = runner.storage.get_player(&id) {
            let status = plr.get_status();
            println!(
                "- {} (id={}): HP={}/{}, move_point:{} ATK={}, DEF={}, SPD={}, AGI={}, MAG={}, MP={}, MDF={}, ITL={}, all_sum={} 系数: {}",
                plr.display_name(),
                id,
                status.hp,
                status.max_hp,
                status.move_point,
                status.attack,
                status.defense,
                status.speed,
                status.agility,
                status.magic,
                status.mp,
                status.resistance,
                status.wisdom,
                status.all_sum,
                plr.get_name_factor()
            );
        }
    }
    println!();
}

fn main() {
    println!(
        "欢迎来到 tswn - {}, 某个充满怨念的人向你问好, 使用 --help/-h 获取帮助信息谢谢喵",
        tswn_core::version()
    );
    println!("WARNING: PRE ALPHA 版本, 仅供测试使用, 已知有 bug, 暂未实现: 天卫、Boss、武器");
    println!("发现行为不一致请不要惊慌, 呼叫 shenjack 即可 (qq: 3695888)");
    let raw = match read_raw_input() {
        Ok(raw) => raw,
        Err(err) => {
            eprintln!("输入错误: {err}");
            print_usage();
            std::process::exit(2);
        }
    };

    let mut runner = match Runner::new_from_namerena_raw(raw) {
        Ok(runner) => runner,
        Err(err) => {
            eprintln!("构建对局失败: {err}");
            std::process::exit(1);
        }
    };

    print_all_players(&runner);

    let mut round = 1usize;
    let mut idle_rounds = 0usize;
    let mut total_score = 0u64;
    let mut score_by_caster: HashMap<usize, u64> = HashMap::new();

    while !runner.have_winner() && round <= 100_000 {
        let updates = runner.main_round();
        if updates.updates.is_empty() {
            idle_rounds += 1;
            if idle_rounds > 16 {
                break;
            }
            continue;
        }
        idle_rounds = 0;

        println!("=== 回合 {round} ===");
        for update in updates.updates {
            match update.update_type {
                UpdateType::NextLine => println!(),
                _ => {
                    if update.score > 0 {
                        total_score += update.score as u64;
                        *score_by_caster.entry(update.caster).or_insert(0) += update.score as u64;
                    }
                    println!("{}", fmt_update(&runner, &update));
                }
            }
        }
        round += 1;
    }

    println!("\n=== 对局结果 ===");
    if let Some(winners) = runner.world.winner.clone() {
        println!("赢家:");
        for winner in winners {
            if let Some(plr) = runner.storage.get_player(&winner) {
                let battle_score = score_by_caster.get(&winner).copied().unwrap_or(0);
                println!(
                    "- {} (id={}, all_sum={}, battle_score={}, hp={})",
                    plr.display_name(),
                    winner,
                    plr.get_status().all_sum,
                    battle_score,
                    plr.get_status().hp
                );
            }
        }
    } else {
        println!("未分出胜负（达到安全轮次或连续空更新）。");
    }
    println!("总战斗分: {total_score}");
}
