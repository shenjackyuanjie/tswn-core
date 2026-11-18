# 等效熟练度实验报告

整理日期为 2026-11-19。实验以 `67c6f757` 上的核心为基线，使用 Windows 上的 `release-fast`、`no_debug` 和 Python 3.14。DIY 字段与导出方法见 [DIY / OL 格式](../reference/diy-overlay.md#按等效熟练度抽取主动技)。

本轮采用 **35 个技能主值 + 10 个动态占位参数**。十项评分共用一套 45 维输入，构造新号只读取原技能组。48 个留出样本的十项平均绝对误差从 40 维版的 **0.583** 降到 **0.484** 个百分点，样本内最大误差从 **3.13** 降到 **2.41** 个百分点。逐项和分组构造在这批样本中均未降低平均误差。

## 样本和评分口径

| 阶段 | 名单 | 起始轮次 | 每项随机对局 | 每个 CQD 靶子 | 每个配对组合 |
| --- | --- | ---: | ---: | ---: | ---: |
| 40 维全名单对照 | 377 个去重名字 | 262144 | 16384 | 384 | 48 |
| 原版扫描统计 | 从 `sqp5900.txt` 抽 120 个独立名字 | 65536 | 12288 | 160 | 80 |
| 重点号加密统计 | `cqd-001`、`cqd-011`、`sqp-062` | 524288 | 65536 | 2048 | 384 |
| 45 维完整复测 | 留出的 48 个名字 | 1048576 | 16384 | 384 | 32 |
| 字段顺序验证 | 377 个去重名字 | 1332736 | 256 | 8 | 2 |

40 维全名单对照包含 CQD 靶池的 50 个名字和 `sqp6000.txt` 的 329 个名字，跨表去重后为 377 个。353 个名字的十项误差均在 2 个百分点以内。最差样本 `sqp-062` 的 CQD 偏差为 6.66 个百分点，扩大样本并换轮次复测后 CQD、QD 仍各差约 5.8 个百分点。本轮的 48 名留出结果使用较小样本，表中的最大误差只适用于对应名单。

扫描统计从 `sqp5900.txt` 中排除上述 377 个名字后抽样，原版累计约 1,646 万局。每个配对预设抽四组队友采集门槛与选目标数据，完整复测则遍历所有预设队友和靶子。实验固定名单抽样为 `Random(1729)`、初选队友为 `Random(596)`。从 377 个名字中抽 96 个，交替分为候选筛选和完整复测两组。全程按原轮次生成数字对手或 seed，完整复测使用独立区间。

| 评分 | 对局环境 | 权重与汇总 |
| --- | --- | --- |
| `qp`、`pp` | 随机队友和对手的 2v2 | `!`、`U+0002` 两种 profile，万分制 |
| `qd`、`pd` | 随机对手的 1v1 | 同上两种 profile，万分制 |
| `cqd` | 固定靶池的 1v1 | 单人靶池加权平均 |
| `fp`、`cp`、`wcp`、`fsp`、`pjp` | 固定队友与固定靶池的 2v2 | 辅评、刺评、无刺评、分身评、配件评各自取带权前四名 |

计算跨项误差时，万分制差值除以 100，CQD 直接取百分比分差，配对总分差除以预设的 `head = 4`。配对选手和队友逐人冻结，同阵容镜像按 50% 计。以下结果以绝对差值的百分点口径报告。预设文件与执行器见 [Openbox 预设](../../setting/settings.toml)和 [评分入口](../../crates/tswn_core/examples/effective_skill_scores.rs)。

## 固定折减系数

观察器只跟踪待评名字及其分身，按本体、分身和智能状态记数。扫描到技能时累加 `min(当前等级, 128)` 作为名义机会，通过状态门槛时同时累加可用机会。随机判定通过后的尝试再分为成功选目标、目标失败两类。门槛和目标失败各自记录后续是否选中另一技能，扫完仍无技能则进入普攻分支。净化即使无状态可清除也会占用这一手，仍计为成功选技。

初次采样按组内座次识别待评号，阵容排序可能让观察器跟到队友。修正后按身份键识别待评号，表中的计数和完整复测均来自修正后的流程。

固定折减系数沿用 SP2 的递推形式。每个评分环境先对该技能汇总计数，估计 `a`，再取十个环境的等权均值。这个值衡量技能占用当前行动机会的比例。

```text
a = 可用机会 / 名义机会 × 成功选目标次数 / 随机判定通过次数
w = R × min(L, 128)
R ← R × (1 - a × min(L, 128) / 128)
```

| 技能 | 环境等权 a | 逐号逐环境 P10–P90 | 本轮用途 |
| --- | ---: | ---: | --- |
| 净化 `disperse` | 1.000 | 1.000–1.000 | 固定系数 |
| 聚气 `accumulate` | 0.583 | 0.552–0.804 | 固定系数 |
| 吸血 `absorb` | 0.691 | 0.584–0.793 | 动态 |
| `half` | 0.740 | 0.705–0.819 | 动态 |
| 换血 `exchange` | 0.502 | 0.307–0.651 | 动态 |
| 治疗 `heal` | 0.635 | 0.543–0.695 | 动态 |
| 复活 `revive` | 0.159 | 0.000–0.277 | 动态 |
| 铁壁 `iron` | 0.683 | 0.551–0.944 | 动态 |
| 蓄力 `charge` | 0.717 | 0.679–0.852 | 动态 |
| 潜行 `assassinate` | 0.799 | 0.750–0.846 | 动态 |
| 分身 `clone` | 1.000 | 1.000–1.000 | 保留扫描位置 |
| 幻术 `shadow` | 0.906 | 0.837–0.947 | 动态 |

分位数只使用名义机会至少 500 次的「名字 × 环境」样本。分位数与环境等权均值的聚合权重不同。对动态技能，表中的 `a` 用于比较失效率，构造时保存熟练度和占位比例，运行时直接用当时的等级、状态门槛与选目标结果。

重点号 `Tik_Tok #IBxWzGZtr@Shabby_fish` 约跑了 75 万局。将同类环境的原始计数合并后，换血的系数在 `qd/pd`、`cqd`、`qp/pp` 和配对环境分别为 **0.176、0.321、0.334、0.395**。其中 `qd/pd` 掷中 37,985 次，6,691 次找到目标。31,294 次目标失败中有 23,927 次转到其他技能，7,367 次转到普攻分支。这些计数描述行动去向。整场胜率影响见独立复测。

## 动态占位与线性模型

`effective_skill_adjustment` 中的 `c_i` 表示原扫描顺序中，位于技能 `i` 前面的普通技能消耗量占整个普通技能池的比例。从只含单参数主动技的剩余量 `R` 递推中，取 `(1 − R_before) / (1 − R_end)`。普通技能池为空时基值取零，相邻动态技能用百万分位保留顺序。

只看一个双参数技能。若普通技能池当前的发动概率为 `P`、该技能轮到自身时的发动概率为 `q`，首抽选中该技能的概率是 `q × (1 − c_i × P)`。固定 `P` 和 `q` 时它对 `c_i` 是仿射关系。跨战况时，状态门槛改变 `P`，技能衰减改变 `q`，多个双参数技能还会按位置互相竞争。选目标失败会复用同一次随机样本继续找候选。

训练线性模型时，可在同一特征预算下对比用 `c_i × q_0` 替换 `c_i`，其中 `q_0 = min(初始熟练度, 128) / 128`。这仍占一个训练维度，近似覆盖乘积项的一部分。它属于待验证的特征变换，当前导出与模拟均使用原始 `c_i`。

分身的 `a` 在采样中稳定为 1。它何时抢占普通技能池仍影响本体及分身衰减后的发动次数，因此保留占位。在 `Accio #LD5MtqO8@Shabby_fish` 上，去掉分身占位后 CQD 偏差从 **+0.59** 变成 **−10.31** 个百分点。用聚气替换该维也得到同样的极端偏差。

## 复测结果与采用方案

| 方案 | 数值维度 | 十项平均绝对误差 | 复测最大绝对误差 |
| --- | ---: | ---: | ---: |
| 上一版，共享系数 | 40 | 0.5826 | 3.1250 |
| **新版，共享系数** | **45** | **0.4845** | **2.4135** |
| 按 `qp/qd/pp/pd` 与 `cqd + pair` 分两套 | 每套 45 | 0.4856 | 2.4135 |
| 按 1v1 / 2v2 分两套 | 每套 45 | 0.4863 | 2.4135 |
| 按随机 1v1、CQD、全部 2v2 分三套 | 每套 45 | 0.4865 | 2.4135 |
| 再拆开随机 2v2 和固定配对，分四套 | 每套 45 | 0.4871 | 2.4135 |
| 每项评分独立一套系数 | 每套 45 | 0.4873 | 2.4173 |
| 共享系数，移除分身占位 | 44 | 0.6047 | 10.3115 |
| 共享系数，移除分身与铁壁占位 | 43 | 0.6424 | 10.3115 |
| 共享系数，分身占位换成聚气 | 45 | 0.6038 | 10.3115 |

共享 45 维方案有 44/48 个名字的十项误差均在 2 个百分点以内，40 维对照为 42/48。新版最差一项为 `test-092` 的分身评。各分组方案使用同一批双参数技能，只变更普通技能固定系数的聚合环境。这个实验支持当前训练使用一份向量，对更大名单的逐号上界以及线性模型的实际误差仍待验证。

在独立小轮次检查中，377 个名字的显式 DIY 各自反转 `skills`、`effective_skills` 和 `effective_skill_adjustment` 字段顺序，3,770 项评分比较全部相同。另直接比较自动构建与逐号冻结的显式 DIY 时，随机评分和 CQD 全部相同，五项配对有 112 项不同。配对中直接传 `effective_skills:true` 会按当局组队加成后的技能组计算。训练与评分对照应使用从同一单号构建导出的显式 DIY。

## 复现

从仓库根目录执行。将本地未跟踪的 `sqp5900.txt` 和 `sqp6000.txt` 放到 `tests/`。留出池按 CQD 单人靶、SQP 依次合并和去重。以下 SHA-256 用 UTF-8、LF、去除空行并保留末尾换行的文本计算。

| 输入 | 行数 | SHA-256 |
| --- | ---: | --- |
| `sqp5900.txt` | 3684 | `5daed6834766eb5885d13073cfd1ed304adebcb67a98342abdfdfeb66126d537` |
| `sqp6000.txt` | 329 | `bf167f45e6abe5539793966f4d6c739b6a0fa7880f88408a4fae806795de8414` |
| `holdout.txt` | 377 | `2a6439848ac7ca8dee97879f5812d456b2f925fa9d975ffd5f6ddef533c631a1` |

```powershell
cargo build --profile release-fast -p tswn_core --features no_debug,effective-skill-stats --bin tswn-cli --example effective_skill_scores
@'
import sys
from pathlib import Path

sys.path.insert(0, "scripts")
from experiment_effective_skills import cases, spec

out = Path("target/effective-skills-report")
out.mkdir(parents=True, exist_ok=True)
cqd = [row["players"][0] for row in spec()["cqd_targets"]]
sqp = cases(Path("tests/sqp6000.txt"))
holdout = list(dict.fromkeys(cqd + sqp))
(out / "holdout.txt").write_text("\n".join(holdout) + "\n", encoding="utf-8")
focus = [cqd[0], cqd[10], sqp[61]]
(out / "focus.txt").write_text("\n".join(focus) + "\n", encoding="utf-8")
'@ | python -

$out = 'target/effective-skills-report'
python scripts/profile_effective_skills.py tests/sqp5900.txt "$out/holdout.txt" $out observe
python scripts/profile_effective_skills.py tests/sqp5900.txt "$out/holdout.txt" $out focus --focus "$out/focus.txt"
python scripts/profile_effective_skills.py tests/sqp5900.txt "$out/holdout.txt" $out confirm `
  --eval-base-n 16384 --eval-cqd-n 384 --eval-pair-n 32 `
  --variants current,shared-dynamic,shared-no-clone,shared-eight,shared-accumulate,user,format,three,four,per-metric
```

复现时 `current` 由脚本显式重建上一版 40 维参数。原始复测使用上一版运行器的自动构造对照。输出包括原版扫描计数 `native-skill-counts.csv`、重点号 `focus-skill-counts.csv`、各环境系数 `coefficients.json`、离散度 `dispersion.json`、复测明细 `confirm-errors.csv` 及汇总 `confirm-summary.json`。原始实验输出在本地的 `target/effective-skills-40/` 和 `target/effective-skills-profile-v3/`。
