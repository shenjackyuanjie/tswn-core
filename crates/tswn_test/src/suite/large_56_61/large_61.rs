use super::*;

/// 火球只要叠加火焰标记就会出问题
/// 第一次中火球不会炸
/// 第二次就炸了
/// 异常偏高
pub fn large_61<E: crate::EngineAdapter>() {
    const CASE: &str = r#"5CZilPr9w=w1b3M@XJ联队
子子油渍柚不子油不是子柚渍不不渍柚油柚子@柚子不是油渍
luogu.com.cn/paste/q9h8sdzk@Shabby_fish
TCO g]Ov>0_I^oG^@流浪冒险者
lyeURjAY@coat

权计WN13vmJnn@candle
虚空托腮 UMOXFIARH@TigerStar
Shinohara_akari TXZIQmGuGF@🥒
三气归来 fg7puGTD2raj@Afterglow
江南小子 #LLVKEPMU@暗黑突击
seed:1376-5-2@!


luogu.com.cn/paste/q9h8sdzk使用魅惑, 江南小子被魅惑了

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 江南小子使用伤害反弹, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到26点伤害

权计WN13vmJnn开始聚气, 权计WN13vmJnn攻击力上升

TCO发起攻击, 江南小子受到58点伤害

虚空托腮发起攻击, lyeURjAY受到71点伤害

lyeURjAY使用火球术, 虚空托腮受到99点伤害

Shinohara_akari使用加速术, 江南小子进入疾走状态

luogu.com.cn/paste/q9h8sdzk使用分身, 出现一个新的luogu.com.cn/paste/q9h8sdzk

江南小子使用火球术, 江南小子受到71点伤害

 江南小子从魅惑中解除

5CZilPr9w=w1b3M使用地裂术

 江南小子受到53点伤害

 Shinohara_akari受到27点伤害

 三气归来受到27点伤害

 权计WN13vmJnn回避了攻击

 Shinohara_akari发起反击, 5CZilPr9w=w1b3M受到44点伤害

江南小子发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到104点伤害

三气归来潜行到TCO身后

权计WN13vmJnn发起攻击, lyeURjAY受到97点伤害

TCO潜行到Shinohara_akari身后

luogu.com.cn/paste/q9h8sdzk使用魅惑, Shinohara_akari被魅惑了

lyeURjAY发起攻击, 虚空托腮受到105点伤害

Shinohara_akari发起攻击, 虚空托腮受到68点伤害

 Shinohara_akari从魅惑中解除

虚空托腮发动铁壁, 虚空托腮防御力大幅上升

江南小子发起攻击, lyeURjAY受到85点伤害

 江南小子从疾走中解除

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 虚空托腮受到1点伤害

luogu.com.cn/paste/q9h8sdzk使用魅惑, 权计WN13vmJnn回避了攻击

江南小子使用火球术, lyeURjAY受到80点伤害

 lyeURjAY被击倒了

Shinohara_akari使用净化, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到44点伤害

权计WN13vmJnn使用地裂术

 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到72点伤害

 TCO受到83点伤害

 TCO的潜行被识破

 5CZilPr9w=w1b3M受到34点伤害

 luogu.com.cn/paste/q9h8sdzk受到52点伤害

luogu.com.cn/paste/q9h8sdzk发起攻击, 江南小子受到48点伤害

三气归来发动背刺, TCO受到396点伤害

 TCO被击倒了

5CZilPr9w=w1b3M使用地裂术

 虚空托腮受到1点伤害

 江南小子受到37点伤害

 三气归来受到28点伤害

 Shinohara_akari受到26点伤害

权计WN13vmJnn使用地裂术

 5CZilPr9w=w1b3M受到100点伤害

 luogu.com.cn/paste/q9h8sdzk受到120点伤害

 luogu.com.cn/paste/q9h8sdzk做出垂死抗争, luogu.com.cn/paste/q9h8sdzk所有属性上升

 luogu.com.cn/paste/q9h8sdzk受到16点伤害

luogu.com.cn/paste/q9h8sdzk发起攻击, 虚空托腮受到1点伤害

虚空托腮使用治愈魔法, 江南小子回复体力70点

三气归来潜行到5CZilPr9w=w1b3M身后

5CZilPr9w=w1b3M发起攻击, Shinohara_akari防御, Shinohara_akari受到31点伤害

luogu.com.cn/paste/q9h8sdzk使用苏生术, lyeURjAY复活了, lyeURjAY回复体力68点

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 虚空托腮受到1点伤害

Shinohara_akari使用加速术, 三气归来进入疾走状态

虚空托腮使用治愈魔法, 虚空托腮回复体力107点

 虚空托腮从铁壁中解除

江南小子发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到66点伤害

三气归来发动背刺, 5CZilPr9w=w1b3M受到293点伤害

 5CZilPr9w=w1b3M被击倒了

 三气归来吞噬了5CZilPr9w=w1b3M, 三气归来属性上升

三气归来潜行到luogu.com.cn/paste/q9h8sdzk身后

lyeURjAY发起攻击, Shinohara_akari受到91点伤害

三气归来发动背刺, luogu.com.cn/paste/q9h8sdzk受到443点伤害

 luogu.com.cn/paste/q9h8sdzk被击倒了

 三气归来吞噬了luogu.com.cn/paste/q9h8sdzk, 三气归来属性上升

 三气归来从疾走中解除

Shinohara_akari发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到44点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子发动铁壁, 子子油渍柚不子油不是子柚渍不不渍柚油柚子防御力大幅上升

luogu.com.cn/paste/q9h8sdzk使用火球术, 江南小子受到137点伤害

 江南小子做出垂死抗争, 江南小子所有属性上升

虚空托腮发动铁壁, 虚空托腮防御力大幅上升

江南小子使用火球术, luogu.com.cn/paste/q9h8sdzk守护子子油渍柚不子油不是子柚渍不不渍柚油柚子, luogu.com.cn/paste/q9h8sdzk受到38点伤害

 luogu.com.cn/paste/q9h8sdzk被击倒了

权计WN13vmJnn使用地裂术

 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

 lyeURjAY受到105点伤害

 lyeURjAY被击倒了

Shinohara_akari使用加速术, 权计WN13vmJnn进入疾走状态

三气归来使用地裂术

 子子油渍柚不子油不是子柚渍不不渍柚油柚子回避了攻击

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 江南小子受到44点伤害

 江南小子被击倒了

权计WN13vmJnn开始蓄力

权计WN13vmJnn使用地裂术

 子子油渍柚不子油不是子柚渍不不渍柚油柚子的铁壁被打消了, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到280点伤害

 子子油渍柚不子油不是子柚渍不不渍柚油柚子被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-61 must contain a blank separator between input and trace",
        "sampled case-61 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 5468, "large_61 score mismatch");
    assert!(guard < 20_000, "sampled case-61 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-61", &actual_lines, &expected_lines);
}
