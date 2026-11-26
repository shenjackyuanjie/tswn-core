# Rosmontic/namerena 的自定义 Boss 清单

当前移植参考：https://github.com/Rosmontic/namerena/blob/2ff2bbb700d3e42c0293c870277efa7fd2ab3aa2/md5.js 。这是带 lazy 编码常量的 Dart2JS 编译产物。根目录 md5.js 与 namerena.github.io/master、touhou/md5.js 不是同一版本，不能混用。

用户指定的 30 个 Boss 均有自定义 16×16 PNG。原图保存在 assets/rosmontic-boss-icons/，来源、文件摘要见 manifest.json。scripts/rosmontic/extract-icons.cjs 可重建图标与核心引擎使用的 RGBA 数据。图标接口、战斗回放和胜率图导出共用这些像素；普通同名角色不受影响。

机制当前已接入琪露诺、冴月麟、露米娅、megia，以及博丽灵梦、雾雨魔理沙、大妖精、红美铃、小恶魔、帕秋莉、十六夜咲夜、蕾米莉亚，共 12 个；其余 18 个尚未接入战斗，不能以图标迁移完成代替机制完成。下面是参考站的历史角色清单，不代表全部已实现。

共有 78 个自定义 Boss：Revue Starlight 10 个、东方 Project 20 个、侦探歌剧 4 个、突击莉莉/蔚蓝档案等联动若干，另有兵种靶机 A01~D04 与 ironbird、tigershark、hell、dark。

| 输入 | 名称 |
| --- | --- |
| `yeya@!` | 夜冴 |
| `karen@!` | 爱城华恋 |
| `hikari@!` | 神乐光 |
| `mahiru@!` | 露崎真昼 |
| `junna@!` | 星见纯那 |
| `nana@!` | 大场奈奈 |
| `maya@!` | 天堂真矢 |
| `kuro@!` | 西条克洛迪娜 |
| `futaba@!` | 石动双叶 |
| `kaoruko@!` | 花柳香子 |
| `reimu@!` | 博丽灵梦 |
| `marisa@!` | 雾雨魔理沙 |
| `rumia@!` | 露米娅 |
| `daiyousei@!` | 大妖精 |
| `cirno@!` | 琪露诺 |
| `hong@!` | 红美铃 |
| `koakuma@!` | 小恶魔 |
| `patchouli@!` | 帕秋莉 |
| `sakuya@!` | 十六夜咲夜 |
| `remilia@!` | 蕾米莉亚 |
| `flandre@!` | 芙兰朵露 |
| `rin@!` | 冴月麟 |
| `letty@!` | 蕾蒂 |
| `chen@!` | 橙 |
| `alice@!` | 爱丽丝 |
| `phantom@!` | 幽灵乐团 |
| `youmu@!` | 魂魄妖梦 |
| `yuyuko@!` | 西行寺幽幽子 |
| `ran@!` | 八云蓝 |
| `yukari@!` | 八云紫 |
| `lilywhite@!` | 莉莉白 |
| `nue@!` | 封兽鵺 |
| `shellingford@!` | 夏洛克·雪莉福德 |
| `nero@!` | 让崎妮洛 |
| `poirot@!` | 艾露克露·伯顿 |
| `gray@!` | 柯蒂妮娅·格拉卡 |
| `quicksilve@!` | 快银 |
| `targeta@!` | 靶机A |
| `targetb@!` | 靶机B |
| `targetc@!` | 靶机C |
| `targetd@!` | 靶机D |
| `aris@!` | 天童爱丽丝 |
| `yuzu@!` | 花岡柚子 |
| `momori@!` | 才羽桃井 |
| `midori@!` | 才羽绿 |
| `AL1S@!` | 天童爱丽丝 |
| `ribbit1@!` | 宫子 |
| `ribbit2@!` | 咲 |
| `ribbit3@!` | 萌绘 |
| `ribbit4@!` | 美游 |
| `fox1@!` | 雪乃 |
| `fox2@!` | 妮可 |
| `fox3@!` | 胡桃 |
| `fox4@!` | 音葵 |
| `hifumi@!` | 阿慈谷日富美 |
| `azusa@!` | 白洲梓 |
| `A01@!` | A-强袭 |
| `B02@!` | B-均衡 |
| `C03@!` | C-敏捷 |
| `D04@!` | D-抵御 |
| `yuki@!` | 月代雪 |
| `ema@!` | 樱羽艾玛 |
| `hiro@!` | 二阶堂希罗 |
| `meruru@!` | 冰上梅露露 |
| `koko@!` | 泽渡可可 |
| `sherii@!` | 橘雪莉 |
| `hanna@!` | 远野汉娜 |
| `anan@!` | 夏目安安 |
| `noa@!` | 城崎诺亚 |
| `reia@!` | 莲见蕾雅 |
| `miria@!` | 佐伯米莉亚 |
| `nanoka@!` | 黑部奈叶香 |
| `maago@!` | 宝生玛格 |
| `arisa@!` | 紫藤亚里沙 |
| `ironbird@!` | 亚飞 |
| `tigershark@!` | 海虎 |
| `hell@!` | 地狱 |
| `dark@!` | 黑暗 |

## 移植难点

- 只有混淆后的 Dart JS，每个 Boss 的技能、数值、状态与台词都要逐个反编译并人工核对。
- 多数 Boss 带自定义状态机（如 Revue Starlight 的闪耀/再生产），无法直接套用现有 8 个自定义 Boss 的模板。

## 当前范围

当前用户要求完成东方 Project 的前 20 个 Boss（含冴月麟），以及 AL1S、fox1～fox4、ribbit1～ribbit4、megia，共 30 个。继续移植时逐个实现技能、被动与召唤状态，再开放对应的挑战入口。

运行 node scripts/rosmontic/extract-reference.cjs SOURCE_MD5_JS OUTPUT_DIR 可导出各角色实际初始化数值、完整技能对象、顺序和关联 hook。技能 f 是逐项概率等级，不是抽取权重。

## 已接入的额外机制

- 冴月麟：保留参考版 -26 攻击与魔力；普通攻击威力归零；偶数次行动输出四段台词，第八次自行退场；八种强状态免疫等级为 240。
- 露米娅：黑雾以 16 ×（防御 + 抗性）反击，跳过前置防御以避免递归反射；境界线群攻、夜盲症命中附加狂暴、月的阴暗面仅施放两次并分别增加 50 防御和抗性。存活且 HP ≤ 100 时一次性进入五倍攻防魔抗智的毁灭模式。
- megia：虚伪的剑闪锁定目标并蓄力；下一次行动从三次魔法威力中取最大值，HP > 199 时六倍，否则十二倍。高体力开始蓄力时挂受伤中断钩子，跨越血量阈值的伤害也会中断。首次致死恢复 199 HP，之后正常死亡。

本批再接入八个角色：

- 博丽灵梦：梦想天生蓄积、二重结界的小伤害吸收和破盾、梦想封印驱散。
- 雾雨魔理沙：八卦炉能量、两段彗星、属性偷取。
- 大妖精：抵消攻击、支援与复活、琪露诺复活加成、牺牲和妖精召唤。
- 红美铃：气场、连续拳击和守护。
- 小恶魔：逆召唤、书本两次施放、带技能回调攻击的减伤。
- 帕秋莉：五行累积、复合符卡、贤者之石与燃烧。
- 十六夜咲夜：时停、残影和连击。
- 蕾米莉亚：鲜红印记、气场与吸血。

运行态随快照和战斗复用接口复制或重置。行为测试位于 runtime/combat/boss/rosmontic_tests.rs 和 touhou_tests.rs，覆盖招式分支、计时、死亡与整场战斗，以及快慢执行和复用重置的一致性；其余角色继续按当前范围移植。
