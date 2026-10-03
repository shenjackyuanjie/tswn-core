//! Openbox 控件的上下文帮助。

use std::cell::Cell;

const HELP_ICON_SIZE: f32 = 18.0;
const HELP_TOOLTIP_WIDTH: f32 = 380.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HelpTopic {
    DiyExport,
    LiveResults,
    Accuracy,
    NamerAccuracy,
    BatchAccuracy,
    PairAccuracy,
    Threads,
    KeepRq,
    ScorePrecision,
    NamerMetrics,
    NamerNames,
    BatchMatchups,
    BatchPlayers,
    ManualTargets,
    BenchOutput,
    Highlight,
    PairTeammates,
    PairDetails,
    PairScore,
    Ds4Workflow,
    Ds4Thresholds,
    Ds4Storage,
    Ds4Three,
    Ds4Prediction,
    Ds4Battle,
}

impl HelpTopic {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::DiyExport => "导出格式与详情",
            Self::LiveResults => "实时结果与视图",
            Self::Accuracy => "精确度与场数",
            Self::NamerAccuracy => "namer-pf 精确度",
            Self::BatchAccuracy => "cqd/cqp 精确度",
            Self::PairAccuracy => "pair 精确度",
            Self::Threads => "线程设置",
            Self::KeepRq => "不低估短号",
            Self::ScorePrecision => "小数位",
            Self::NamerMetrics => "评分项与技能榜",
            Self::NamerNames => "名字输入",
            Self::BatchMatchups => "每组胜率",
            Self::BatchPlayers => "选手分组",
            Self::ManualTargets => "靶子输入",
            Self::BenchOutput => "输出格式与阈值",
            Self::Highlight => "高亮超强名字",
            Self::PairTeammates => "队友与保留数量",
            Self::PairDetails => "cqp 详情",
            Self::PairScore => "pair 分数与阈值",
            Self::Ds4Workflow => "DS4 筛选流程",
            Self::Ds4Thresholds => "DS4 阈值与实战筛选",
            Self::Ds4Storage => "DS4 工作目录与增量历史",
            Self::Ds4Three => "DS4 三人组合与后处理",
            Self::Ds4Prediction => "DS4 模型环境与诊断",
            Self::Ds4Battle => "DS4 三轮实战筛选",
        }
    }

    pub(crate) fn body(self) -> &'static str {
        match self {
            Self::Ds4Workflow => super::ds4_help::WORKFLOW,
            Self::Ds4Thresholds => super::ds4_help::THRESHOLDS,
            Self::Ds4Storage => super::ds4_help::STORAGE,
            Self::Ds4Three => super::ds4_help::THREE_DETAILS,
            Self::Ds4Prediction => super::ds4_help::PREDICTION,
            Self::Ds4Battle => super::ds4_help::BATTLE,
            Self::DiyExport => {
                "每行输入一个名字或组合，组内成员用 + 分隔。默认导出 +ol；“旧 +diy”和“召唤物diy”是另外两种格式，不能同时开启。\n\n\
                 屏幕详情包括属性、技能、八围与嘲讽。带括号的 + / - 数值表示组队后相对单独构建的变化，并非额外加成选项。\n\n\
                 写入文件时只保存导出行，不包含属性说明。可以先不写文件查看详情，确认后再导出。"
            }
            Self::LiveResults => {
                "纯文本沿用旧版格式：DIY 导出行后接原始信息块，评分显示“名字 指标:分数”，胜率和配队显示“分数 名字”及缩进明细。完整结果块完成后追加，不混入预览前缀；卡片适合展开一组结果；表格适合横向比较，点击名字查看详情。视图选择重启后仍会恢复。\n\n\
                 蓝色“预览”表示明细已算完，但整组尚未结束；绿色“完成”表示该组全部完成；停止或失败时，未算完的组会保留“不完整”标记。高亮结果标红，技能榜用蓝色。\n\n\
                 多线程会让不同组交错完成，#序号对应原输入位置，重名也分别展示。pair 的“当前 Top”会随新队友结果更新，最终排名以完成状态为准。\n\n\
                 明细会先在卡片、表格中预览，整组完成后再按日志阈值筛选；未达标组从这两种视图移除，纯文本只保留通过筛选的结果。namer-pf 的五项评分分别按各自阈值显示，未显示的指标用 — 表示。\n\n\
                 “排版设置”可调卡片左、中、右对齐、表格每列的宽度与对齐，以及表格高度；拖动表头右边界调列宽，拖动表格与明细之间的分隔条调表格高度，表格高度可切回“自动”。卡片与表格的文字使用和纯文本日志相同的等宽字体与行距，三种视图逐行对齐。主题、工具页、视图、跟随、排版、窗口与输入面板尺寸自动保存，重启恢复。勾选“跟随最新”会立即跳到最新；向上滚动暂停跟随，滑回最底部会自动恢复勾选。复制日志总是复制当前保留的纯文本；“复制全部”在表格里复制当前选中行、在卡片里复制当前卡片，每张展开卡片的右下角也有同样的按钮。清空只影响当前展示，不会停止计算；大量历史可能被裁剪，需要完整结果时请开启文件输出。"
            }
            Self::Accuracy => {
                "精确度决定每次对局的模拟场数：\n\
                 1% = 100 场\n\
                 10% = 1,000 场\n\
                 100% = 10,000 场\n\n\
                 场数越高，结果通常越稳定，耗时也越长。选择“场数”可以直接输入自定义数量。"
            }
            Self::NamerAccuracy => {
                "namer-pf 默认使用 100%，即每项评分模拟 10,000 场。\n\n\
                 需要更细的小数分辨率时，可在更多设置中提高自定义场数，并设置输出小数位。"
            }
            Self::BatchAccuracy => {
                "cqd/cqp 中，10% 表示每个选手与每组靶子对战 1,000 场。\n\n\
                 常规数据可以使用 100%；大规模数据可先用 10% 粗筛，再用 100% 复核。粗筛阈值可比正式阈值略低，例如低 1 分，避免漏掉边界结果。"
            }
            Self::PairAccuracy => {
                "pair 通常使用 10%，即每个选手、队友与靶子组合模拟 1,000 场。\n\n\
                 提高精确度会按组合数量成比例增加计算量。"
            }
            Self::Threads => {
                "“系统线程 * 1.5”会让运行时自动分配线程，显示的线程值 0 代表自动模式。\n\n\
                 一般建议保持自动；只有需要限制资源占用或排查性能时再手动设置。"
            }
            Self::KeepRq => {
                "开启后会保留短号的原始 rq，避免按默认规则低估。\n\n\
                 该选项会改变评分口径；与未开启该选项的历史结果比较时，评分可能不一致。"
            }
            Self::ScorePrecision => {
                "控制评分或胜率输出保留的小数位，范围为 0 到 9。\n\n\
                 小数位只控制显示格式；实际能分辨到多细仍取决于模拟场数。"
            }
            Self::NamerMetrics => {
                "勾选“屏幕”或“输出文件”决定该评分项写到哪里。阈值留空表示不限制；填写后，只输出分数不低于阈值的名字。\n\n\
                 “高亮超强名字”是相对屏幕阈值的增量：分数 >= 屏幕阈值 + 增量时，该行会标红；屏幕阈值留空时按 0 计算。\n\n\
                 参考范围：pp 常见约 8,600，pd 约 9,400，qp 约 6,300-6,400，qd 约 7,000-7,200，sum 约 31,000。它们是经验值，不是程序默认阈值。\n\n\
                 技能榜会找出名字中等级最高的技能，并按 setting/score_now.toml 筛选；若待评名字或组合的全部技能熟练度均小于 30，还会按 [lessskl]（白板号）阈值筛选。全能除满足 all 阈值外，还需 pp >= 8,000、pd >= 9,000、qp >= 6,000、qd >= 7,000。"
            }
            Self::NamerNames => {
                "每行输入一个名字或一组名字；组内名字使用 + 分隔。\n\n\
                 可以直接在文本框输入，也可以勾选“从文件中读取”。文件模式只预览前几行，运行时会读取完整文件。"
            }
            Self::BatchMatchups => {
                "开启后，除选手的平均胜率外，还会输出该选手与每组靶子的单独胜率。\n\n\
                 这会显著增加大规模任务的日志量；大量数据粗筛时建议关闭。"
            }
            Self::BatchPlayers => {
                "每行表示一个待测选手或选手组。普通二人组、三人组使用 + 分隔。\n\n\
                 如果名字本身带有 +diy/覆盖层内容，请在更多设置中开启“DIYcqp”，改用 ++ 分隔组员。"
            }
            Self::ManualTargets => {
                "默认从 setting/settings.toml 选择靶子预设。开启“使用手动靶子”后，可以直接输入或从文件读取。\n\n\
                 普通靶子组使用 + 分隔；靶子名字本身含 + 时，开启“DIY靶子”并使用 ++ 分隔。"
            }
            Self::BenchOutput => {
                "日志阈值控制最终结果是否保留在右侧；文件阈值独立控制是否写入输出文件。留空表示不限制，等于阈值的结果也保留。实时明细可能先显示，全部完成后再筛选。\n\n\
                 “分数 名字”适合直接查看；JSONL 适合程序读取；“名字 (--pure)”只输出名字。未选择输出文件时，结果只写入右侧日志。"
            }
            Self::Highlight => {
                "当分数 >= 日志阈值 + 高亮增量时，屏幕中的结果会标红。\n\n\
                 日志阈值留空时按 0 计算。该设置只影响屏幕颜色，不影响文件筛选。"
            }
            Self::PairTeammates => {
                "队友预设来自 setting/settings.toml。请按待测账号属性选择刺评、辅评、无刺评、分身评或配件评。\n\n\
                 预设中的“保留前几”决定最终分数取最高几个 cqp：常见预设取前 5，分身评和配件评通常取前 3。手动队友模式可以单独修改这个数量。\n\n\
                 选手与队友都会先按“单独构建”冻结成 DIY 再组成二人组：同公会（@队名相同）成员之间的组队加成不计入 cqp，两侧都按各自单独构建的强度计算。"
            }
            Self::PairDetails => {
                "不显示 cqp：只显示最终分数。\n\
                 每组 cqp：显示所有达到 cqp 阈值的队友组合；阈值留空表示全部显示。\n\
                 有效 cqp：只显示最终分数实际采用的前几个组合。\n\n\
                 例如 cqp 阈值填写 48 时，只显示 cqp >= 48 的队友组合。这个阈值只影响详情显示，不影响最终分数计算。\n\n\
                 每个队友与全部靶子的计算结束后才显示该队友的 cqp；“有效 cqp”运行中展示当前 Top，所有队友完成后才确定最终排名。"
            }
            Self::PairScore => {
                "pair 最终分数是 cqp 从高到低排序后，前 head 个结果的总和。日志阈值和文件阈值都针对这个总分。\n\n\
                 参考范围：刺评、辅评、无刺评通常约 242-245；分身评、配件评通常约 144-147。它们是经验值，不是程序默认阈值。"
            }
        }
    }
}

pub(crate) fn help_icon(ui: &mut egui::Ui, topic: HelpTopic, requested: &Cell<Option<HelpTopic>>) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(HELP_ICON_SIZE, HELP_ICON_SIZE), egui::Sense::click());
    let color = if response.hovered() {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let painter = ui.painter();
    painter.circle_stroke(rect.center(), 7.0, egui::Stroke::new(1.2_f32, color));
    painter.text(
        rect.center() + egui::vec2(0.0, -0.5),
        egui::Align2::CENTER_CENTER,
        "i",
        egui::FontId::proportional(12.0),
        color,
    );

    let response = response.on_hover_ui(|ui| {
        ui.set_max_width(HELP_TOOLTIP_WIDTH);
        ui.label(egui::RichText::new(topic.title()).strong());
        ui.separator();
        ui.label(topic.body());
        ui.add_space(4.0);
        ui.label(egui::RichText::new("点击图标可固定此说明").weak().small());
    });
    if response.clicked() {
        requested.set(Some(topic));
    }
}

pub(crate) fn show_help_window(ctx: &egui::Context, active: &mut Option<HelpTopic>) {
    let Some(topic) = *active else {
        return;
    };
    let mut open = true;
    egui::Window::new(format!("说明 - {}", topic.title()))
        .id(egui::Id::new("openbox_pinned_help"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(440.0)
        .show(ctx, |ui| {
            ui.set_max_width(520.0);
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                ui.label(topic.body());
            });
        });
    if !open {
        *active = None;
    }
}
