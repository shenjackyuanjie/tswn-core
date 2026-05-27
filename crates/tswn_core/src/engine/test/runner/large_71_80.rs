//! 大型回放测试分片 71-80。
//!
//! 保存由真实/采样输入生成的长回放 fixture，按编号拆分以降低单文件体积并方便定位失败 case。

use super::*;

#[test]
fn large_71() {
    const CASE: &str = r#"南峰 }bRMSYZX@Shabby_fish
反袭 #YN785ClJ3@Shabby_fish


反袭使用幻术, 召唤出幻影

南峰潜行到反袭身后

南峰发动背刺, 反袭受到329点伤害

反袭发起攻击, 南峰受到87点伤害

反袭使用火球术, 南峰回避了攻击

南峰发起攻击, 幻影受到61点伤害

幻影使用附体, 南峰进入狂暴状态

 幻影消失了

南峰发起狂暴攻击, 反袭受到23点伤害

反袭发起攻击, 南峰受到0点伤害

反袭发起攻击, 南峰受到58点伤害

反袭发起攻击, 南峰回避了攻击

南峰发起狂暴攻击, 南峰回避了攻击

反袭发起攻击, 南峰受到39点伤害

南峰发起狂暴攻击, 反袭受到65点伤害

 反袭被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-71 must contain a blank separator between input and trace",
        "sampled case-71 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-71 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-71", &actual_lines, &expected_lines);
}

#[test]
fn large_72() {
    const CASE: &str = r#"来日再会 #ysKNqZlKC@Shabby_fish
Light_Years_Away #XgTW5RYlF@Shabby_fish
Fly_Away #4D6i0uPzI@Shabby_fish

Parallel #JjhUCkk02@Shabby_fish
QwQ ([IaPM!&@Shabby_fish
Rictusempra ^W)-WaDF@Shabby_fish

白雨屠咒句@Shabby_fish
Superpower #ddDROyhTJ@Shabby_fish
废土 #tYHujog1a@Shabby_fish


Fly_Away使用净化, Superpower受到88点伤害

废土发动铁壁, 废土防御力大幅上升

白雨屠咒句发起攻击, Parallel回避了攻击

QwQ潜行到白雨屠咒句身后

来日再会使用幻术, 召唤出幻影

Superpower发起攻击, 来日再会守护Light_Years_Away, 来日再会受到22点伤害

Rictusempra潜行到Fly_Away身后

Parallel潜行到来日再会身后

白雨屠咒句发起攻击, 来日再会受到47点伤害

来日再会发起攻击, Parallel受到72点伤害

 Parallel的潜行被识破

Fly_Away使用净化, 白雨屠咒句受到50点伤害

QwQ发动背刺, 白雨屠咒句受到346点伤害

 白雨屠咒句被击倒了

 QwQ吞噬了白雨屠咒句, QwQ属性上升

废土使用地裂术

 来日再会守护Light_Years_Away, 来日再会受到24点伤害

 来日再会受到17点伤害

 QwQ受到27点伤害

 Rictusempra使用伤害反弹, 废土受到1点伤害

Light_Years_Away开始聚气, Light_Years_Away攻击力上升

Parallel发起攻击, 来日再会受到65点伤害

Superpower发起攻击, Light_Years_Away受到22点伤害

Rictusempra发动背刺, Fly_Away受到329点伤害

 Fly_Away被击倒了

来日再会发起攻击, Rictusempra受到110点伤害

Parallel潜行到Light_Years_Away身后

废土发起攻击, Rictusempra受到64点伤害

 废土从铁壁中解除

QwQ潜行到Light_Years_Away身后

Light_Years_Away发起攻击, Parallel受到139点伤害

 Parallel的潜行被识破

Superpower使用魅惑, QwQ被魅惑了

Rictusempra发起攻击, 废土受到29点伤害

Parallel潜行到Light_Years_Away身后

来日再会使用幻术, 召唤出幻影

Parallel发动背刺, Light_Years_Away受到188点伤害

废土开始聚气, 废土攻击力上升

幻影发起攻击, 废土受到57点伤害

Superpower发起攻击, QwQ受到22点伤害

 QwQ的潜行被识破

QwQ发起攻击, 来日再会受到102点伤害

 QwQ从魅惑中解除

Light_Years_Away使用生命之轮, 废土回避了攻击

废土使用生命之轮, QwQ回避了攻击

Superpower使用分身, 出现一个新的Superpower

Parallel潜行到废土身后

Rictusempra潜行到废土身后

来日再会使用减速术, Superpower进入迟缓状态

Light_Years_Away使用冰冻术, 废土受到59点伤害, 废土被冰冻了

幻影发起攻击, Rictusempra受到57点伤害

 Rictusempra的潜行被识破

Superpower发起攻击, Light_Years_Away守护来日再会, 来日再会守护Light_Years_Away, 来日再会受到12点伤害

QwQ潜行到废土身后

幻影发起攻击, 废土受到57点伤害

Rictusempra发起攻击, 幻影受到75点伤害

Parallel发动背刺, 废土受到464点伤害

 废土被击倒了

 Parallel吞噬了废土, Parallel属性上升

Parallel使用生命之轮, Light_Years_Away的体力值与Parallel互换, Light_Years_Away发动隐匿

QwQ发起攻击, 幻影受到55点伤害

Light_Years_Away使用地裂术

 QwQ受到42点伤害

 Parallel受到39点伤害

 Superpower受到37点伤害

 Superpower受到40点伤害

 Rictusempra受到23点伤害

幻影使用附体, Parallel回避了攻击

Superpower发起攻击, 幻影受到59点伤害

来日再会使用幻术, 召唤出幻影

幻影使用附体, QwQ进入狂暴状态

 幻影消失了

幻影发起攻击, QwQ受到109点伤害

Superpower发起攻击, 来日再会回避了攻击

QwQ发起攻击, 来日再会受到66点伤害

 来日再会被击倒了

 幻影消失了

 幻影消失了

Light_Years_Away使用冰冻术, QwQ防御, QwQ受到28点伤害, QwQ被冰冻了

Parallel开始蓄力

Rictusempra发起攻击, Superpower受到83点伤害

 Superpower被击倒了, Superpower使用护身符抵挡了一次死亡, Superpower回复体力4点

Superpower发起攻击, QwQ受到53点伤害

Light_Years_Away使用地裂术

 Parallel受到50点伤害

 QwQ受到47点伤害

 Rictusempra回避了攻击

 Superpower受到64点伤害

 Superpower被击倒了

Parallel使用地裂术

 Light_Years_Away受到161点伤害

 Light_Years_Away被击倒了

 Superpower受到223点伤害

 Superpower被击倒了"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-72 must contain a blank separator between input and trace",
        "sampled case-72 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-72 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-72", &actual_lines, &expected_lines);
}
