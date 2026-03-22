//! # 名竞 CLI 工具 (namerena_cli)
//!
//! 本模块实现名竞的命令行工具，提供对战、Benchmark、图标生成等功能。
//!
//! ## 功能说明
//!
//! - **对战模式** — 支持从 stdin、命令行参数或文件读取输入，进行对战
//! - **Benchmark 模式** — 自动检测输入组数，支持评分测试和胜率测试
//! - **图标生成** — 支持生成玩家图标、Base64 编码、保存 PNG 文件
//!
//! ## 使用方法
//!
//! ### 对战模式（默认）
//!
//! ```bash
//! # 从 stdin 读取
//! echo "a\nb\n\nc\nd" | namerena_cli
//!
//! # 使用命令行参数
//! namerena_cli --raw "a\nb\n\nc\nd"
//!
//! # 从文件读取
//! namerena_cli --file input.txt
//! ```
//!
//! ### Benchmark 模式
//!
//! ```bash
//! # 评分测试（1组，多线程）
//! echo "mario" | namerena_cli --bench 500
//!
//! # 单线程 benchmark
//! echo "mario" | namerena_cli --bench-st 500
//!
//! # 胜率测试（2+组，多线程）
//! namerena_cli --bench-raw "team1\n\nteam2" 1000
//!
//! # 胜率测试（2+组，单线程）
//! namerena_cli --bench-raw-st "team1\n\nteam2" 1000
//!
//! # 从文件读取
//! namerena_cli --bench-file input.txt 1000
//! ```
//!
//! ### 图标生成
//!
//! ```bash
//! # 显示图标信息
//! namerena_cli --icon mario luigi
//!
//! # 输出 Base64 PNG
//! namerena_cli --icon-b64 mario
//!
//! # 保存 PNG 文件
//! namerena_cli --icon-path ./icons mario luigi
//! ```
//!
//! ## Benchmark 模式说明
//!
//! - **1组输入** → 评分测试（普通评分 + !评分）
//! - **2+组输入** → 胜率测试（team1 vs team2）
//!
//! 评分测试会生成 N 个测试靶（普通评分使用 `\u{0002}` 前缀，!评分使用 `!` 前缀），
//! 统计目标组的胜场数并计算评分。
//!
//! 胜率测试会统计 team1（组0）的胜场数，计算胜率百分比。
//!
//! ## 图标渲染
//!
//! 图标渲染使用 ANSI 真彩色转义码在终端显示彩色方块预览。
//! 支持的边框样式：
//! - 0: `─`
//! - 1: `━`
//! - 2: `═`
//! - 3: `┄`
//! - 4: `┅`
//! - 5: `╌`
//! - 6: `╍`
//!
//! **注意**: `--icon` 功能不需要 `png_render` feature，可以直接使用：
//!
//! ```bash
//! cargo run --bin namerena_cli -- --icon mario
//! ```
//!
//! `--icon-b64` 和 `--icon-path` 功能需要 `png_render` feature：
//!
//! ```bash
//! cargo run --bin namerena_cli --features png_render -- --icon-b64 mario
//! ```
//!
//! ## 示例
//!
//! ```bash
//! # 显示帮助
//! namerena_cli --help
//!
//! # 简单对战
//! echo -e "mario\nluigi\n\npeach\nbowser" | namerena_cli
//!
//! # 评分测试
//! echo "mario" | namerena_cli --bench 1000
//!
//! # 生成图标
//! namerena_cli --icon mario
//! ```
//!
//! ## 限制
//!
//! - PRE ALPHA 版本，仅供测试使用
//! - 已知有 bug
//! - 暂未实现：天卫、Boss、武器

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read};

use tswn_core::Runner;
use tswn_core::engine::update::{RunUpdate, UpdateType};
use tswn_core::player::icon::icon_from_raw_name;

use tswn_core::player::icon_render::render_icon_vec_from_name;

#[cfg(feature = "png_render")]
use tswn_core::player::icon_render::{render_icon_b64_from_name, render_icon_png};

fn print_usage() {
    println!(
        r#"用法:
  namerena_cli [选项]

对战模式（默认）:
  --raw <字符串>         使用提供的原始字符串作为输入（支持 \n 换行）
  --file <文件路径>       从文件读取输入
  --out-raw             输出 raw 聚合战斗日志（仅普通对战模式生效）
  <无参数>               从 stdin 读取（输入格式：用空行分隔队伍）

Benchmark 模式:
  自动检测：1组输入 → 实力评分测试，2+组输入 → 对战胜率测试
  --bench [N]              从 stdin 读取，运行 N 场 (默认 1000，多线程)
  --bench-raw "..." [N]    使用提供的原始字符串（多线程）
  --bench-file <文件> [N]   从文件读取（多线程）
  --bench-st [N]           从 stdin 读取（单线程）
  --bench-raw-st "..." [N] 使用提供的原始字符串（单线程）
  --bench-file-st <文件> [N] 从文件读取（单线程）

胜率测试（简化版）:
  --win_rate <队伍1> <队伍2> [N]     两队对战，多线程 (默认 1000)
  --win_rate_st <队伍1> <队伍2> [N]  两队对战，单线程 (默认 1000)

性能测试:
  --perf <队伍1> <队伍2> [N]      性能基准测试，运行 N 场 (默认 10000)，输出 init/fight 耗时分解

图标功能:
  --icon <名字>...             输出玩家图标信息（终端渲染预览）
  --icon-b64 <名字>...         输出图标的 base64 PNG 数据 URL [需要 png_render feature]
  --icon-path <目录> <名字>... 将图标 PNG 保存到 <目录>/<名字>.png [需要 png_render feature]

其他:
  --help, -h                   显示此帮助信息

调试环境变量:
  TSWN_DEBUG_ACTION=<名字>   调试特定玩家的行动
  TSWN_DEBUG_STATS           调试玩家属性计算
  TSWN_DEBUG_WORLD           调试世界状态同步
  TSWN_DEBUG_TICK            调试每个 tick 的执行
  TSWN_DEBUG_PICK            调试目标选择逻辑
  TSWN_DEBUG_DODGE           调试闪避逻辑
  TSWN_DEBUG_DODGE_ALL       调试所有玩家的闪避
  TSWN_DEBUG_DIE             调试死亡处理
  TSWN_DEBUG_STATE           调试状态系统（状态设置/清除/追踪）
  TSWN_DEBUG_COVID           调试 COVID Boss 相关逻辑
  TSWN_DEBUG_FIRE            调试火焰技能
  TSWN_DEBUG_HEAL            调试治疗技能
  TSWN_DEBUG_UPGRADE=<名字>  调试升级技能
  TSWN_DEBUG_REFLECT         调试反射技能
  TSWN_TRACE_RC4             追踪 RC4 随机数状态
  TSWN_BENCH_WORKERS=<N>     benchmark 线程数覆盖（并行模式）
  TSWN_WINRATE_WORKERS=<N>   兼容旧变量名，作用同上

输入格式说明:
  - 用空行分隔不同队伍
  - 每行一个玩家名称
  - 示例：
    mario
    luigi

    peach
    bowser

示例:
  # 对战
  namerena_cli --raw "a\nb\n\nc\nd"
  namerena_cli --out-raw --raw "a\nb\n\nc\nd"
  echo -e "mario\nluigi\n\npeach\nbowser" | namerena_cli

  # 评分测试
  echo "mario" | namerena_cli --bench 500
  echo "mario" | namerena_cli --bench-st 500

  # 胜率测试
  namerena_cli --bench-raw "team1\n\nteam2" 1000
  namerena_cli --bench-raw-st "team1\n\nteam2" 1000
  namerena_cli --win_rate "mario" "luigi" 1000
  namerena_cli --win_rate_st "mario" "luigi" 1000

  # 带调试信息
  TSWN_DEBUG_ACTION=mario namerena_cli --raw "mario\nluigi""#
    );
}

fn read_raw_input(args: &[String]) -> Result<String, String> {
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
        "--icon-b64" => {
            #[cfg(feature = "png_render")]
            {
                if args.len() < 2 {
                    eprintln!("--icon-b64 需要至少一个名字参数");
                    std::process::exit(2);
                }
                for name in &args[1..] {
                    let b64 = render_icon_b64_from_name(name);
                    if args.len() == 2 {
                        // 单个名字时直接输出 data URL
                        println!("{b64}");
                    } else {
                        println!("{name}: {b64}");
                    }
                }
                std::process::exit(0);
            }
            #[cfg(not(feature = "png_render"))]
            {
                eprintln!("错误: --icon-b64 需要 png_render feature");
                eprintln!("请使用: cargo run --bin namerena_cli --features png_render -- --icon-b64 <名字>");
                std::process::exit(1);
            }
        }
        "--icon-path" => {
            #[cfg(feature = "png_render")]
            {
                if args.len() < 3 {
                    eprintln!("--icon-path 需要: <目录> <名字> [更多名字...]");
                    std::process::exit(2);
                }
                let dir = std::path::Path::new(&args[1]);
                if let Err(e) = fs::create_dir_all(dir) {
                    eprintln!("创建目录失败: {e}");
                    std::process::exit(1);
                }
                for name in &args[2..] {
                    let path = dir.join(format!("{name}.png"));
                    let icon = icon_from_raw_name(name);
                    let png = render_icon_png(&icon);
                    if let Err(e) = fs::write(&path, &png) {
                        eprintln!("写入 {} 失败: {e}", path.display());
                        std::process::exit(1);
                    }
                    println!("已保存: {}", path.display());
                }
                std::process::exit(0);
            }
            #[cfg(not(feature = "png_render"))]
            {
                eprintln!("错误: --icon-path 需要 png_render feature");
                eprintln!("请使用: cargo run --bin namerena_cli --features png_render -- --icon-path <目录> <名字>");
                std::process::exit(1);
            }
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

fn collect_args_with_flags() -> (Vec<String>, bool) {
    let mut out_raw = false;
    let mut args = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--out-raw" {
            out_raw = true;
        } else {
            args.push(arg);
        }
    }
    (args, out_raw)
}

fn is_bench_like_command(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some(
            "--bench"
                | "--bench-st"
                | "--bench-raw"
                | "--bench-raw-st"
                | "--bench-file"
                | "--bench-file-st"
                | "--win_rate"
                | "--win_rate_st"
                | "--perf"
        )
    )
}

fn is_non_fight_command(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("--help" | "-h" | "--icon" | "--icon-b64" | "--icon-path")
    ) || is_bench_like_command(args)
}

/// 打印给定玩家名称的图标 TUI 表示。
fn print_icon(name: &str) {
    let icon = icon_from_raw_name(name);
    let [br, bg, bb] = icon.bg_color;

    println!("=== Icon: {name} ===");
    println!("边框样式: {}", icon.border_style);
    println!("形状: {:?}", icon.shapes);
    println!("背景色: #{:02X}{:02X}{:02X} (索引 {})", br, bg, bb, icon.bg_color_idx);

    // 渲染 RGBA 像素数据到终端
    let pixels = render_icon_vec_from_name(name);
    render_pixels_to_terminal(&pixels);

    // 前景色详情
    for (i, (idx, color)) in icon.fg_color_indices.iter().zip(icon.fg_colors.iter()).enumerate() {
        let [r, g, b] = *color;
        println!(
            "前景色 {i}: \x1b[48;2;{r};{g};{b}m    \x1b[0m #{r:02X}{g:02X}{b:02X} (索引 {idx}, 形状 {})",
            icon.shapes[i]
        );
    }
    println!();
}

/// 将 RGBA 像素数据渲染到终端，使用 ANSI 块字符实现 1:1 渲染
fn render_pixels_to_terminal(pixels: &[u8]) {
    // 绘制边框
    let border_line = "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━";
    println!("┌{}┐", border_line);

    // 使用 ANSI 块字符渲染
    // 每个终端字符显示 2 个横向像素，补偿终端字符的宽高比
    for y in 0..16 {
        print!("│");
        for x in 0..16 {
            // 获取像素颜色
            let pixel = get_pixel(pixels, x, y);

            if let Some((r, g, b)) = pixel {
                // 使用前景色和块字符
                print!("\x1b[38;2;{r};{g};{b}m██\x1b[0m");
            } else {
                print!("  ");
            }
        }
        println!("│");
    }

    println!("└{}┘", border_line);
}

/// 获取指定位置的像素颜色 (RGBA)
fn get_pixel(pixels: &[u8], x: usize, y: usize) -> Option<(u8, u8, u8)> {
    if x >= 16 || y >= 16 {
        return None;
    }
    let idx = (y * 16 + x) * 4;
    if idx + 3 >= pixels.len() {
        return None;
    }
    let r = pixels[idx];
    let g = pixels[idx + 1];
    let b = pixels[idx + 2];
    let a = pixels[idx + 3];
    if a == 0 {
        None // 透明像素
    } else {
        Some((r, g, b))
    }
}

fn plr_name(runner: &Runner, id: usize) -> String {
    runner
        .storage
        .get_player(&id)
        .map(|plr| plr.display_name())
        .unwrap_or_else(|| format!("#{id}"))
}

fn fmt_update_with_mode(runner: &Runner, update: &RunUpdate, append_score: bool) -> String {
    let caster = plr_name(runner, update.caster);
    let target = plr_name(runner, update.target);
    let targets = if let Some(p) = update.param {
        p.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update.targets.iter().map(|id| plr_name(runner, *id)).collect::<Vec<String>>().join(",")
    };

    let mut msg = update.message.to_string();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    msg = msg.replace("[2]", &targets);
    if append_score && update.score > 0 {
        format!("{msg}  (+{})", update.score)
    } else {
        msg
    }
}

fn fmt_update(runner: &Runner, update: &RunUpdate) -> String { fmt_update_with_mode(runner, update, true) }

fn fmt_update_raw(runner: &Runner, update: &RunUpdate) -> String { fmt_update_with_mode(runner, update, false) }

fn sanitize_output_line(line: &str) -> String {
    let filtered = line
        .chars()
        .filter(|ch| !ch.is_control() && !matches!(*ch, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'))
        .collect::<String>();

    let mut normalized = String::with_capacity(filtered.len());
    let mut prev_space = false;
    for ch in filtered.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                normalized.push(' ');
                prev_space = true;
            }
        } else {
            normalized.push(ch);
            prev_space = false;
        }
    }
    normalized.trim().to_string()
}

fn is_action_line(line: &str) -> bool {
    line.contains("发起攻击")
        || (line.contains("使用") && !line.contains("护身符抵挡了一次死亡"))
        || line.contains("做出垂死抗争")
        || line.contains("连击")
        || line.contains("从疾走中解除")
}

fn emit_current_turn(output_lines: &mut Vec<String>, pending_action_line: &mut String, pending_misc_lines: &mut Vec<String>) {
    if !pending_action_line.is_empty() {
        output_lines.push(std::mem::take(pending_action_line));
        output_lines.push(String::new());
        pending_misc_lines.clear();
        return;
    }
    if !pending_misc_lines.is_empty() {
        output_lines.push(pending_misc_lines.join(", "));
        output_lines.push(String::new());
        pending_misc_lines.clear();
    }
}

fn print_fight_raw(runner: &mut Runner) {
    let mut output_lines: Vec<String> = Vec::new();
    let mut pending_action_line = String::new();
    let mut pending_misc_lines: Vec<String> = Vec::new();

    let mut round = 1usize;
    let mut idle_rounds = 0usize;
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

        for update in updates.updates {
            if matches!(update.update_type, UpdateType::NextLine) {
                emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
                continue;
            }

            let line = sanitize_output_line(&fmt_update_raw(runner, &update));
            if line.is_empty() {
                continue;
            }

            if is_action_line(&line) {
                emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
                pending_action_line = line;
                continue;
            }

            if pending_action_line.is_empty() {
                pending_misc_lines.push(line);
            } else {
                pending_action_line.push_str(", ");
                pending_action_line.push_str(&line);
            }
        }
        round += 1;
    }

    emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
    while matches!(output_lines.last(), Some(line) if line.is_empty()) {
        output_lines.pop();
    }

    if !output_lines.is_empty() {
        println!("{}", output_lines.join("\n"));
    }
}

// ─────────────────────────── Benchmark ───────────────────────────────────────

/// 性能测试：输出详细的 init/fight/total 耗时分解。
fn run_perf(team1: &str, team2: &str, n: usize) {
    let raw = format!("{team1}\n\n{team2}");
    println!("=== 性能测试 ({n} 场) ===");
    println!("team1: {team1}  team2: {team2}");

    let mut wins = 0usize;
    let mut total = 0usize;
    let mut init_nanos = 0u128;
    let mut fight_nanos = 0u128;

    // warmup
    for i in 0..10 {
        let bench_input = format!("{raw}\n\nseed:warmup{i}@!");
        if let Ok(mut runner) = Runner::new_from_namerena_raw(bench_input) {
            runner.run_to_completion();
        }
    }

    let t_total = std::time::Instant::now();

    for i in 0..n {
        let bench_input = format!("{raw}\n\nseed:{i}@!");

        let t_init = std::time::Instant::now();
        let mut runner = match Runner::new_from_namerena_raw(bench_input) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let team0_roster: Vec<usize> = runner.world.teams.first().map(|t| t.roster.clone()).unwrap_or_default();
        init_nanos += t_init.elapsed().as_nanos();

        let t_fight = std::time::Instant::now();
        runner.run_to_completion();
        fight_nanos += t_fight.elapsed().as_nanos();

        total += 1;
        if let Some(ref w) = runner.world.winner
            && w.iter().any(|id| team0_roster.contains(id))
        {
            wins += 1;
        }
        if (i + 1) % 1000 == 0 {
            eprint!("\r进度: {}/{n}  ", i + 1);
        }
    }

    let total_elapsed = t_total.elapsed();
    eprint!("\r                    \r");
    let _ = std::io::Write::flush(&mut std::io::stderr());

    let rate = wins as f64 * 100.0 / total.max(1) as f64;
    let n_f = total.max(1) as f64;
    println!("胜率: {:.2}%  ({}/{})", rate, wins, total);
    println!("─────────────────────────────────");
    println!(
        "total :  {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
        total_elapsed.as_secs_f64(),
        total_elapsed.as_micros() as f64 / n_f,
        n_f / total_elapsed.as_secs_f64()
    );
    println!(
        "init  :  {:.3}s  ({:.1}µs/场)",
        init_nanos as f64 / 1e9,
        init_nanos as f64 / 1e3 / n_f
    );
    println!(
        "fight :  {:.3}s  ({:.1}µs/场)",
        fight_nanos as f64 / 1e9,
        fight_nanos as f64 / 1e3 / n_f
    );
}

/// Benchmark 入口：根据输入组数自动选择模式。
/// - 2+ 组 → 胜率（team1 vs team2）
/// - 1 组  → 普通评分 + !评分
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BenchThreadMode {
    Parallel,
    SingleThread,
}

fn resolve_bench_workers(mode: BenchThreadMode, total: usize) -> usize {
    match mode {
        BenchThreadMode::SingleThread => 1,
        BenchThreadMode::Parallel => {
            let default_workers = std::thread::available_parallelism()
                .map(|x| x.get().saturating_mul(5).div_ceil(4))
                .unwrap_or(1);
            std::env::var("TSWN_BENCH_WORKERS")
                .ok()
                .and_then(|raw| raw.parse::<usize>().ok())
                .filter(|value| *value > 0)
                .or_else(|| {
                    std::env::var("TSWN_WINRATE_WORKERS")
                        .ok()
                        .and_then(|raw| raw.parse::<usize>().ok())
                        .filter(|value| *value > 0)
                })
                .unwrap_or(default_workers)
                .min(total.max(1))
        }
    }
}

fn run_benchmark(raw: &str, n: usize, mode: BenchThreadMode) {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let group_count = groups.iter().filter(|g| !g.is_empty()).count();
    match group_count {
        0 => eprintln!("benchmark: 输入为空或无有效玩家"),
        1 => run_bench_score(raw, n, mode),
        _ => run_bench_winrate(raw, n, mode),
    }
}

/// 胜率测试：team1（组0）vs team2（组1），跑 n 场，统计组0胜率。
fn run_bench_winrate(raw: &str, n: usize, mode: BenchThreadMode) {
    println!("=== 对战胜率测试 ({n} 场) ===");
    let t_start = std::time::Instant::now();
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let team0_count = groups
        .first()
        .map(|group| group.iter().filter(|name| !tswn_core::player::Player::check_is_seed(name)).count())
        .unwrap_or(0);
    let groups = std::sync::Arc::new(groups);
    let workers = resolve_bench_workers(mode, n);

    let (wins, total) = if workers <= 1 || n < 2000 {
        run_bench_winrate_range(groups.as_ref(), team0_count, 0, n)
    } else {
        let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let groups = std::sync::Arc::clone(&groups);
            let next = std::sync::Arc::clone(&next);
            handles.push(std::thread::spawn(move || {
                run_bench_winrate_worker(groups.as_ref(), team0_count, next.as_ref(), n)
            }));
        }
        let mut merged = (0usize, 0usize);
        for handle in handles {
            let (w, t) = handle.join().expect("winrate worker thread panicked");
            merged.0 += w;
            merged.1 += t;
        }
        merged
    };

    let elapsed = t_start.elapsed();
    let rate = wins as f64 * 100.0 / total.max(1) as f64;
    println!("胜率: {:.2}%  ({}/{})", rate, wins, total);
    println!(
        "耗时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
        elapsed.as_secs_f64(),
        elapsed.as_micros() as f64 / total.max(1) as f64,
        total as f64 / elapsed.as_secs_f64()
    );
}

fn run_bench_winrate_range(groups: &[Vec<String>], team0_count: usize, start: usize, end: usize) -> (usize, usize) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut seed = String::with_capacity(24);

    for i in start..end {
        seed.clear();
        let _ = write!(&mut seed, "seed:{i}@!");
        let seed_ref = std::slice::from_ref(&seed);
        let mut runner = match Runner::new_from_groups_with_seed(groups, seed_ref) {
            Ok(r) => r,
            Err(_) => continue,
        };
        runner.run_to_completion();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.iter().any(|winner| *winner < team0_count)
        {
            wins += 1;
        }
    }
    (wins, total)
}

fn run_bench_winrate_worker(
    groups: &[Vec<String>],
    team0_count: usize,
    next: &std::sync::atomic::AtomicUsize,
    end: usize,
) -> (usize, usize) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut seed = String::with_capacity(24);

    loop {
        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if i >= end {
            break;
        }
        seed.clear();
        let _ = write!(&mut seed, "seed:{i}@!");
        let seed_ref = std::slice::from_ref(&seed);
        let mut runner = match Runner::new_from_groups_with_seed(groups, seed_ref) {
            Ok(r) => r,
            Err(_) => continue,
        };
        runner.run_to_completion();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.iter().any(|winner| *winner < team0_count)
        {
            wins += 1;
        }
    }
    (wins, total)
}

/// 评分测试：目标组 vs N 个测试靶，跑 n 场，返回 (胜场数, 总场数)。
///
/// - `modifier = "\u{0002}"` → Test1 靶（普通评分）
/// - `modifier = "!"` → TestEx 靶（!评分）
fn run_bench_score_inner(
    target_str: &str,
    target_count: usize,
    modifier: &str,
    n: usize,
    mode: BenchThreadMode,
) -> (usize, usize) {
    let workers = resolve_bench_workers(mode, n);
    if workers <= 1 || n < 2000 {
        return run_bench_score_range(target_str, target_count, modifier, 0, n, true);
    }

    let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let target_str = target_str.to_string();
        let modifier = modifier.to_string();
        let next = std::sync::Arc::clone(&next);
        handles.push(std::thread::spawn(move || {
            run_bench_score_worker(target_str.as_str(), target_count, modifier.as_str(), next.as_ref(), n)
        }));
    }

    let mut merged = (0usize, 0usize);
    for handle in handles {
        let (w, t) = handle.join().expect("score worker thread panicked");
        merged.0 += w;
        merged.1 += t;
    }
    merged
}

fn run_bench_score_range(
    target_str: &str,
    target_count: usize,
    modifier: &str,
    start: usize,
    end: usize,
    show_progress: bool,
) -> (usize, usize) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut targets = String::with_capacity(target_count.saturating_mul(24));
    let mut bench_input = String::with_capacity(target_str.len() + target_count.saturating_mul(24) + 3);

    for i in start..end {
        targets.clear();
        let base = tswn_core::engine::PROFILE_START as usize + i * target_count;
        for offset in 0..target_count {
            if offset > 0 {
                targets.push('\n');
            }
            let _ = write!(&mut targets, "{}@{modifier}", base + offset);
        }

        bench_input.clear();
        bench_input.push_str(target_str);
        bench_input.push_str("\n\n");
        bench_input.push_str(&targets);

        let mut runner = match Runner::new_from_namerena_raw(bench_input.clone()) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let team0_roster: Vec<usize> = runner.world.teams.first().map(|t| t.roster.clone()).unwrap_or_default();

        runner.run_to_completion();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.iter().any(|w| team0_roster.contains(w))
        {
            wins += 1;
        }
        if show_progress && (i + 1) % 100 == 0 {
            print!("\r  进度: {}/{}  ", i + 1, end);
        }
    }
    if show_progress {
        println!();
    }
    (wins, total)
}

fn run_bench_score_worker(
    target_str: &str,
    target_count: usize,
    modifier: &str,
    next: &std::sync::atomic::AtomicUsize,
    end: usize,
) -> (usize, usize) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut targets = String::with_capacity(target_count.saturating_mul(24));
    let mut bench_input = String::with_capacity(target_str.len() + target_count.saturating_mul(24) + 3);

    loop {
        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if i >= end {
            break;
        }
        targets.clear();
        let base = tswn_core::engine::PROFILE_START as usize + i * target_count;
        for offset in 0..target_count {
            if offset > 0 {
                targets.push('\n');
            }
            let _ = write!(&mut targets, "{}@{modifier}", base + offset);
        }

        bench_input.clear();
        bench_input.push_str(target_str);
        bench_input.push_str("\n\n");
        bench_input.push_str(&targets);

        let mut runner = match Runner::new_from_namerena_raw(bench_input.clone()) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let team0_roster: Vec<usize> = runner.world.teams.first().map(|t| t.roster.clone()).unwrap_or_default();

        runner.run_to_completion();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.iter().any(|w| team0_roster.contains(w))
        {
            wins += 1;
        }
    }
    (wins, total)
}

/// 评分测试入口：同时跑普通评分和 !评分。
fn run_bench_score(raw: &str, n: usize, mode: BenchThreadMode) {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let target_group = groups.into_iter().next().unwrap_or_default();
    let target_count = target_group.len();
    if target_count == 0 {
        eprintln!("评分: 无目标玩家");
        return;
    }
    let target_str = target_group.join("\n");

    println!("=== 实力评分测试 ({n} 场) ===");
    println!("目标: {}", target_group.join(", "));

    println!("info: {target_count}");
    eprint!("[普通评分] ");
    let (nw, nt) = run_bench_score_inner(&target_str, target_count, "\u{0002}", n, mode);
    let ns = nw as f64 * 10_000.0 / nt.max(1) as f64;
    println!("普通评分: {:.0} / 10000  ({nw}/{nt})", ns);

    eprint!("[!评分]    ");
    let (bw, bt) = run_bench_score_inner(&target_str, target_count, "!", n, mode);
    let bs = bw as f64 * 10_000.0 / bt.max(1) as f64;
    println!("!评分:     {:.0} / 10000  ({bw}/{bt})", bs);
}

// ─────────────────────────── 普通对战 ────────────────────────────────────────

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
    let (args, out_raw_requested) = collect_args_with_flags();
    let out_raw = out_raw_requested && !is_non_fight_command(&args);

    if !out_raw {
        println!("欢迎来到 tswn - {}, 使用 --help/-h 获取帮助信息谢谢喵", tswn_core::version());
        println!("WARNING: ALPHA 版本, 仅供测试使用, 已知有 bug, 暂未实现: 天卫、Boss、武器");
        println!("发现行为不一致请不要惊慌, 呼叫 shenjack 即可 (qq: 3695888)（欢迎入群 hack: 935216900）");
    }

    // ── Benchmark 模式优先检测 ──────────────────────────────────────────────
    if !args.is_empty() {
        match args[0].as_str() {
            "--bench" | "--bench-st" => {
                let n = args.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1000);
                let mode = if args[0] == "--bench-st" {
                    BenchThreadMode::SingleThread
                } else {
                    BenchThreadMode::Parallel
                };
                let mut raw = String::new();
                if let Err(e) = io::stdin().read_to_string(&mut raw) {
                    eprintln!("读取 stdin 失败: {e}");
                    std::process::exit(2);
                }
                run_benchmark(raw.trim(), n, mode);
                return;
            }
            "--bench-raw" | "--bench-raw-st" => {
                if args.len() < 2 {
                    eprintln!("{} 需要一个字符串参数", args[0]);
                    std::process::exit(2);
                }
                let raw = args[1].replace("\\n", "\n");
                let n = args.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1000);
                let mode = if args[0] == "--bench-raw-st" {
                    BenchThreadMode::SingleThread
                } else {
                    BenchThreadMode::Parallel
                };
                run_benchmark(raw.trim(), n, mode);
                return;
            }
            "--bench-file" | "--bench-file-st" => {
                if args.len() < 2 {
                    eprintln!("{} 需要一个文件路径参数", args[0]);
                    std::process::exit(2);
                }
                let raw = match fs::read_to_string(&args[1]) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("读取文件失败: {e}");
                        std::process::exit(2);
                    }
                };
                let n = args.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1000);
                let mode = if args[0] == "--bench-file-st" {
                    BenchThreadMode::SingleThread
                } else {
                    BenchThreadMode::Parallel
                };
                run_benchmark(raw.trim(), n, mode);
                return;
            }
            "--win_rate" | "--win_rate_st" => {
                if args.len() < 3 {
                    eprintln!("{} 需要 <team1> <team2> [N] 参数", args[0]);
                    std::process::exit(2);
                }
                let team1 = &args[1];
                let team2 = &args[2];
                let n = args.get(3).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1000);
                let raw = format!("{team1}\n\n{team2}");
                let mode = if args[0] == "--win_rate_st" {
                    BenchThreadMode::SingleThread
                } else {
                    BenchThreadMode::Parallel
                };
                run_bench_winrate(&raw, n, mode);
                return;
            }
            "--perf" => {
                if args.len() < 3 {
                    eprintln!("--perf 需要 <team1> <team2> [N] 参数");
                    std::process::exit(2);
                }
                let team1 = &args[1];
                let team2 = &args[2];
                let n = args.get(3).and_then(|s| s.parse::<usize>().ok()).unwrap_or(10000);
                run_perf(team1, team2, n);
                return;
            }
            _ => {}
        }
    }

    let raw = match read_raw_input(&args) {
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

    if out_raw {
        print_fight_raw(&mut runner);
        return;
    }

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
