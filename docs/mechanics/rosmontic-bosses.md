# Rosmontic/namerena 的自定义 Boss 清单（待移植）

来源：https://github.com/Rosmontic/namerena （只有编译产物 touhou/index.dart.js 与 lang/*.json，没有 Dart 源码）。

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

## 建议顺序

1. 先做一个主题（推荐 Revue Starlight 10 个）。
2. 每个 Boss：数值/技能/状态 → Rust 模块 → 测试 → 文档。
3. 完成后统一决定是否加回挑战下拉。
