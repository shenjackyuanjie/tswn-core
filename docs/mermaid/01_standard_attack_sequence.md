# 标准攻击时序图（Standard attack sequence）

说明：本图展示常见的攻击处理主线 —— 从单位行动触发技能，到目标被攻击、执行防御、计算伤害并处理受伤/死亡的完整调用顺序。
注意：本图仅表示“标准攻击流程”（即通过 `attacked(...)` 入口触发），不包含直接调用 `defend(...)`、直接修改 `hp`、自爆/直接 `onDie`、召唤物伤害转移等特例。那些特例请参见对应的分图文件。

```mermaid
sequenceDiagram
    autonumber
    participant C as 控制器 (Controller)
    participant A as 攻击者 (Attacker)
    participant S as 技能/动作 (Skill)
    participant T as 受击者 (Target)
    participant R as 消息队列 (RunUpdates)
    participant G as 队伍 (Grp)

    %% 回合入口与技能发动
    C->>+A: step() / action() (执行步骤)
    A->>+S: select & act (选择目标并行动)
    S->>+T: attacked (受击入口: atp, isMag...)

    %% 目标：predefend（可多项、按注册顺序）
    note right of T: predefends: 修改攻击力或产生副作用(如反弹)
    T->>T: 依次执行 predefends 回调列表
    
    alt predefend 返回 atp == 0 (完全抵挡 / 已反弹)
        T-->>S: return (无伤害，可能已触发反伤逻辑)
        R->>C: push RunUpdate (记录反弹或无效化信息)
    else 继续流程
        %% 闪避判定
        T->>T: Alg.dodge(...) 闪避判定?
        alt 闪避成功 (dodge == true)
            T->>R: push RunUpdate.dodge (记录闪避消息)
            %% 【修复关键点 2】此处 T-->>S (去掉了中间的负号，保持激活状态)
            T-->>S: return (攻击被闪避)
        else 闪避失败 (命中)
            %% 防御与伤害计算
            T->>T: defend (计算防御与基础伤害)
            note right of T: defend -> Alg.getDf -> 计算 dmg
            %% postdefends 可修改 dmg
            T->>T: 执行 postdefends (修正最终伤害)
            T->>R: push RunUpdate.damage (记录伤害数值/文本)
            %% onDamage 回调与实际扣血
            T->>T: damage (实际扣减 HP)
            T->>T: 执行 postdamages 回调
            T->>T: onDamaged (受伤后处理: oldhp...)

            alt hp <= 0 (死亡判定)
                T->>T: onDie (死亡回调)
                T->>T: 执行 dies 回调列表
                T->>G: Grp.die(T) (从队伍移除)
                G->>C: checkWin() (检查胜负)
            end
            %% 【注意】最后一个结束点通常可以保留 -，表示停用 Target
            T-->>-S: return (受击处理完毕)
        end
    end

    S-->>-A: act() returns (动作结束)
    A->>R: collect updates (收集更新数据)
    R-->>C: broadcast / render (广播更新或渲染日志)
```
