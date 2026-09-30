//! 对应上游 Readme.md、README_3.md 与实际 Rust 行为的 DS4 上下文帮助。

pub const DIRECTORY: &str = "选择包含 config.json 和 input/ 的工作目录；支持中文、空格和括号。新目录使用默认配置。手动修改路径后先读取配置，运行前自动保存。";
pub const TEAM: &str = "team_name 必填。每行最后一个 @ 后须以此队伍名开头；评分等尾缀会被截断，行尾空白会被忽略。不匹配、缺少 @ 或格式非法的行写入 out/ignore_input.txt。";
pub const THREADS: &str = "thread_number：结合 CPU 核心数和可用内存设置。ABCP5 启动失败时依次尝试请求线程数、一半、四分之一和 1；降低线程只影响速度，不改变模型或阈值。";
pub const DEDUP: &str =
    "run_dup：与 file/old.txt 对照，只保留新增输入。此选项不清除缓存；常规增量处理应开启，并保留 file/ 历史状态。";
pub const SINGLE: &str =
    "基础分类分别判断评分 sieve 与潜力 ptt_sieve：任一达到对应阈值即保留，不要求同时达标。一个名字可以进入多个分类。";
pub const SP1: &str =
    "控制这一项 SP1 单人评分及其输出；sieve 是最低保留分数。QP/QD/PP 还独立筛选技能结果，PD/CQD 没有技能容差设置。";
pub const SKILL: &str = "skill_sieve 是技能容差：分数 >= 对应技能基线 - 容差时保留，越大越宽松。优先读取工作目录 score_now.txt（108 个数值，36 × 3），没有该文件则使用内置基线；结果追加到 out/qp_skill.txt、qd_skill.txt、pp_skill.txt。";
pub const ABCP: &str = "abcp.enable 控制普通二人 ABCP5 预测，abcp.sieve 是最终输出最低分数，与三人入口阈值独立。已评测组合（包括未过筛的）缓存到 file/two_old.txt；只预测 tmp/two_new.txt，再合并旧结果排序。需要 ABCP5 模型和运行库。";
pub const THREE: &str = "get_3 是三人总开关；还需在更多设置中启用各类型。二人组先过 ABCP5 入口阈值，再按 int(score * 2 + 200) 换算分数并归类到 FC/WC/RH，同一组合可能属于多类。普通二人预测开关不控制此流程。";
pub const BASE: &str = "three.pair_abcp_sieve：二人组进入三人流程前的 ABCP5 最低分，也决定 Openbox 三轮阈值。整数除以 100 得 base，三轮分别为 base+1、base+2、base+2；4400 对应 45%、46%、46%，等于阈值也保留。";
pub const OPENBOX: &str = "默认关闭。从 abcp5/result_without_score.txt 读取组合，直接调用 Openbox Rust 后端，按 100 / 1000 / 10000 场复核。使用工作目录 setting/settings.toml 的预设 2（默认带权 newTarget2），最终追加到 file/real_two.txt。";
pub const COPY_SCORES: &str =
    "copy_pf_to_out：把本轮单项评分写入 out/new_qp.txt 等文件，按分数从高到低排序。技能筛选结果另行追加，不受此选项控制。";
pub const COPY_NEW: &str = "copy_to_new：把本轮分类保存到 new/，便于检查或后续使用；file/ 则保存后续增量计算使用的累计历史。";
pub const RUN: &str = "运行前自动保存 config.json。阶段耗时差异较大，进度百分比不代表剩余时间。当前按整轮执行，暂不支持中途取消，请等待归档完成后关闭窗口。";

pub const WORKFLOW: &str = "首次使用：选择工作目录，填写队伍名，把原始名字文件放入 input/，保存并运行。之后添加新名字即可继续增量处理，不要清空 file/。\n\n单人评分生成 BC/FZ/WC/FS/PJ 分类及 SP1 单项结果；二人阶段生成 FC（FZ × BC）、WC（WC × WC）、RH（FS × PJ）。\n\n二人和三人增量都只计算 new × new、new × old、old × new，不重复计算 old × old；完全相同的字符串不组成二人组。输出顺序可能受多线程影响，不构成结果语义。\n\n三人流程先预测并重新分类二人组，再与单人配对；普通二人 ABCP5 输出是另一个独立开关。只做单人和基础二人配对时，可关闭这两项。\n\nOpenbox 实战筛选继续复核 ABCP5 输出。各阶段完成后归档历史；界面进度只表示阶段数，当前不支持中途取消。";
pub const THRESHOLDS: &str = "基础分类 BC/FZ/WC/FS/PJ：评分 >= sieve 或潜力 >= ptt_sieve，满足其一即保留。\n\nSP1 的 QP/QD/PP/PD/CQD 以及二人、三人的 sieve 是最低保留分数，越高通常保留越少；这些分数不等于实战胜率，不能直接横向比较。\n\nQP/QD/PP 的 skill_sieve 是技能容差，越大反而越宽松：分数 >= 技能基线 - 容差。工作目录 score_now.txt（108 个数值，36 × 3）可覆盖内置基线；技能结果追加到 out/<type>_skill.txt。PD/CQD 不提供技能筛选。\n\nabcp.sieve 只控制普通二人预测的最终输出；three.pair_abcp_sieve 控制三人入口预测及实战筛选门槛，两者不要混淆。关闭类型开关后，该类型的阈值不参与本轮计算。";
pub const STORAGE: &str = "选择目录后读取 config.json，新目录使用默认配置；手动修改路径后请先读取。保存时保留未识别的扩展字段，运行前自动保存。\n\ninput/：原始名字文件，不要直接把原始输入放入 file/。\nout/：FC/WC/RH 二人结果、new_qp.txt 等排序后的单项结果、追加的技能结果、ignore_input.txt。\n3ren/：八类三人结果。\nfile/：old.txt 历史输入、分类状态、预测缓存和 real_two.txt 实战结果。\nnew/：本轮分类；tmp/：临时输入与筛选文件。\n\nfile/two_old.txt 保存所有已预测二人组，包括未过筛组合；tmp/two_new.txt 是本轮新预测输入。abcp5/result.txt 合并历史结果后排序，result_without_score.txt 去掉分数。三人预测另用 file/FC_old.txt、WC_old.txt、RH_old.txt 缓存。\n\n常规增量运行不要清空历史。改变模型或阈值后若需重评旧数据，请新建工作目录并放入原始输入；关闭“历史去重”不能代替重建预测缓存。";
pub const THREE_DETAILS: &str = "get_3 与各类型 enable 共同控制生成，sieve 是三人最低输出分数：\nFFC = FZ × FC；WFC = WC × FC\nFWC = FZ × WC；WWC = WC × WC\nRWC = FS × WC；RRH = FS × RH\nPRH = PJ × RH；WRH = WC × RH\n结果分别追加到 3ren/<TYPE>.txt。\n\n二人先过 three.pair_abcp_sieve，再按 int(ABCP分数 * 2 + 200) 换算并重新分类，同一组合可以进入多个类型。增量分类使用 new/new_three_<TYPE>.txt 与 file/old_three_<TYPE>.txt，three_ 前缀避免 Windows 大小写路径碰撞。\n\n内部单人分类格式是“两列分数 + 名字”，二人分类格式是“一列分数 + 两名字”；原始 input/ 不需要这些分数列。\n\n本页面生成三人结果，尚未整合上游后处理工具。若另有上游程序，可用 extract_high.exe 默认每类取前 40000，再用 sort.exe 0 去分数、duplicate.exe 对无分数的 A+B+C 行忽略成员顺序去重。";
pub const PREDICTION: &str = "ABCP5 需要 abcp5.exe、model4.onnx、scale.txt 以及配套 ONNX / VC++ 运行库，不依赖 Python。优先使用环境变量 TSWN_DS4_ABCP_DIR 指定目录，否则使用工作目录 abcp5/。\n\n支持中文、空格和括号路径。启动失败会按请求线程数、一半、四分之一、1 依次重试；只改变资源占用，不改变模型、阈值和计算规则。\n\n失败时保留输入与现场；Rust 入口会写入 *.failure.txt 诊断，具体路径见错误日志。检查程序、模型和 DLL 是否齐全，再调整线程数或修复运行环境。不要把 C++ 的诊断文件名当成 Rust 固定路径。";
pub const BATTLE: &str = "默认关闭。输入为 abcp5/result_without_score.txt；直接调用 Openbox crate，无需外部 openbox-cli.exe。使用工作目录 setting/settings.toml 中的靶子预设 2，默认对应带权 newTarget2。\n\n令 base = three.pair_abcp_sieve / 100（整数除法）：\n第 1 轮：100 场，阈值 base + 1，输出无分数组合；\n第 2 轮：1000 场，阈值 base + 2，输出无分数组合；\n第 3 轮：10000 场，阈值 base + 2，输出带胜率结果。\n例如 4400 对应 45% / 46% / 46%；等于阈值保留。关闭逐靶明细，每轮临时文件覆盖写入 tmp/。\n\n最终结果追加到 file/real_two.txt。成功归档后清空 ABCP5 input.txt、result.txt、result_without_score.txt；失败保留现场供诊断和重试。";

pub fn pair(key: &str) -> &'static str {
    match key {
        "two_fc" => {
            "FC = FZ × BC，结果写入 out/FC.txt。开关控制配对，sieve 是最低保留分数；只配 new/new、new/old、old/new，排除相同字符串。"
        }
        "two_wc" => {
            "WC = WC × WC，结果写入 out/WC.txt。开关控制配对，sieve 是最低保留分数；只配 new/new、new/old、old/new，排除相同字符串。"
        }
        _ => {
            "RH = FS × PJ，结果写入 out/RH.txt。开关控制配对，sieve 是最低保留分数；只配 new/new、new/old、old/new，排除相同字符串。"
        }
    }
}

pub fn three(key: &str) -> String {
    let combination = match key {
        "ffc" => "FZ × FC",
        "wfc" => "WC × FC",
        "fwc" => "FZ × WC",
        "wwc" => "WC × WC",
        "rwc" => "FS × WC",
        "rrh" => "FS × RH",
        "prh" => "PJ × RH",
        _ => "WC × RH",
    };
    format!(
        "{} = {combination}，结果追加到 3ren/{}.txt。需同时启用三人总开关；sieve 是三人最低输出分数。只计算包含本轮新增单人或二人组的组合。",
        key.to_uppercase(),
        key.to_uppercase()
    )
}
