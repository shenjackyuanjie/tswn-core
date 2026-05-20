# namerena 但是 rust 版

> 坏了, 我真开始技术封锁了

- 即日起 tswn 源码开启 Early Access 活动! (不是)
  - 要源码给我 github id 就行
- 我宣布我与 Github Copilot 的缘分到此为止!
  - 唉，GitHub 也烧不起钱了
  - 先是砍 Github Edu
  - 再是砍 Copilot Pro
  - 战至最后一刻，自刎归天得了(不是)
- DeepSeek V4 也是发了
  - DeepSeek 牛逼!
  - 50% 的 1M MRCR v2 啊
  - 不过 V4 预览版后训练还是差点意思
    - DeepSeek 大人拿着 800B 的 V3 都能吭哧吭哧后训练一年
    - 1.6T 的 V4 pro 估计也能训个一年半载的
  - 20260429

- 高考后开源~
  - 20260322

- Claude Sonnet 4.6 也挺好用的
  - 完成了 icon 部分
- 20260308

- 项目最大赞助商: Github Copilot Pro ( 甚至是 Github Edu 送的 )
  - 我敲，你懂什么叫 Claude Opus 4.6 跑了快一整天还只消耗了一次额度的震撼吗.png
  - 这太TM震撼了
  - 甚至它会自动 compat
    - 还是免费的 ( 不消耗额度 )
  - 此处稍微 cue 一下 GPT 5.4, 他在 context 快用完的时候会自己停下来
    - 倒不是说不好吧，但是在这种项目里自己停下 == 消耗另一次额度
  - 所以 Claude Opus 4.6 就是超级大救星.png
- 20260307

- 感谢 GPT 5.3-Codex Xhigh & Cluade Opus 4.6 两位大哥
- 成功的把项目推进到了 debug 抠细节阶段
- 感谢 minimax m2.5, 写了个 `track_test.py`
  - 非常好的测试工具
  - 当然有个小 bug, 如果 `cargo test` 没跑起来 (比如网炸了) 他也会认为是 pass
    - 当然这种情况属于酒吧点炒饭，也没什么好追究的
  - 然后感谢 Claude Opus 4.6
  - 改进了测试工具, 添加了存档点功能
- 现在也有 `track_case_miner.py`
  - 专门追踪 `tswn_case_miner` 产出的 failed case 集合和 `first_mismatch_idx`
  - 还有个统一薄封装 `track.py`
- 感谢 DouBao Seed 2.0-code
  - 成功把 case 07+10 推进了几个 idx
  - 还是免费的!
- 非常不感谢 qwen3.5-plus-2026-02-15
  - 烧了我 `当月未结清 ¥ 152.74` 块钱还一点进度没有
- 项目已经成 AI benchmark 了捏
  - 无敌的DeepSeek V4大人
  - 快带着你无敌的定价
  - 无敌的1M上下文注意力
  - 还有无敌的SOTA成绩
  - 创翻这个沟槽的项目
  - 也顺手把无能的 GPT 5.3-codex xhigh & Claude Opus 4.6 创飞吧😭
- 目前用过的 AI (按照我自己爱怎么排怎么排):
  - GPT 5.3-Codex Xhigh
  - Claude Opus 4.6
  - MiniMax M 2.5
  - DouBao Seed 2.0-code
  - qwen3.5-plus-2026-02-15
  - GPT 5 mini (好哦，是免费模型)
  - GPT 5.2
  - GPT 5.2-Codex
  - DeepSeek V3.2 (好哦，是DeepSeek)
- 目前花销：
  - Qwen3.5-plus-2026-02-15: 152.74 RMB
  - Claude Opus 4.6: 50 RMB (在中转站充的)
  - GPT 5.3-Codex Xhigh: 50 RMB (让哥们帮忙所以在中转站充的)
- 20260228

- SortInt 结束, 可以开始哐哐写逻辑了
  - 2025 04 16 00:52

- 我也不知道这个库啥时候会公开
- 但是我尽快吧
  - 20240521 ( 00:13 )
- 不过我估计不会公开, 毕竟名竞这玩意的活力也不大
- 我这玩意要是公开了，直接就是一个灭顶之灾
- 当然，在各位名竞 oier 骂我之前, 我先忏悔, 我太狂妄了(笑)

> shenjack
> @夜冴 啊？

> @shenjack 隐匿的判定在狂暴效果生效之后，在没有return的情况下顶掉了狂暴效果，此时本体仍处于狙暴效果中，但可以正常行动，所以需要在此添增加一次对狂暴效果的判定
