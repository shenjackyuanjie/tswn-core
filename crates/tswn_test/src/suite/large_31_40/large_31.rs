use super::*;

pub fn large_31<E: crate::EngineAdapter>() {
    const CASE: &str = r####"nXOINTHn5q
emuuGEJFCU
9Qjf75FlDX
X5nHALQsd8
dLZ78AtLlO
GtZHyiydqU
PDET6mWWde
HnTM6ax71z
KzNAjCWSH7
dpCNNufW7B


dLZ78AtLlO开始蓄力

dpCNNufW7B发起攻击, emuuGEJFCU受到60点伤害

emuuGEJFCU使用冰冻术, KzNAjCWSH7受到60点伤害, KzNAjCWSH7被冰冻了

9Qjf75FlDX发起攻击, dLZ78AtLlO受到75点伤害

X5nHALQsd8发起攻击, 9Qjf75FlDX受到56点伤害

PDET6mWWde发起攻击, X5nHALQsd8受到49点伤害

nXOINTHn5q发起攻击, GtZHyiydqU受到35点伤害

HnTM6ax71z发动铁壁, HnTM6ax71z防御力大幅上升

GtZHyiydqU使用瘟疫, X5nHALQsd8回避了攻击

dpCNNufW7B发起攻击, X5nHALQsd8受到68点伤害

emuuGEJFCU发起攻击, GtZHyiydqU受到97点伤害

dLZ78AtLlO发起攻击, KzNAjCWSH7受到361点伤害

 KzNAjCWSH7被击倒了

9Qjf75FlDX发起攻击, emuuGEJFCU受到94点伤害

X5nHALQsd8发起攻击, dLZ78AtLlO受到59点伤害

HnTM6ax71z发起攻击, GtZHyiydqU受到78点伤害

GtZHyiydqU使用雷击术

 dLZ78AtLlO受到20点伤害

 dLZ78AtLlO受到40点伤害

 dLZ78AtLlO受到19点伤害

 dLZ78AtLlO受到23点伤害

nXOINTHn5q发动会心一击, X5nHALQsd8回避了攻击

dpCNNufW7B发起攻击, PDET6mWWde受到81点伤害

PDET6mWWde发起攻击, GtZHyiydqU受到35点伤害

emuuGEJFCU发起攻击, HnTM6ax71z受到1点伤害

dLZ78AtLlO发起攻击, emuuGEJFCU受到78点伤害

9Qjf75FlDX发起攻击, dpCNNufW7B受到133点伤害

X5nHALQsd8发起攻击, HnTM6ax71z受到1点伤害

dpCNNufW7B发起攻击, 9Qjf75FlDX受到33点伤害

emuuGEJFCU发起攻击, dLZ78AtLlO受到80点伤害

 dLZ78AtLlO被击倒了

HnTM6ax71z发起攻击, X5nHALQsd8受到81点伤害

 HnTM6ax71z从铁壁中解除

GtZHyiydqU发起攻击, HnTM6ax71z受到95点伤害

9Qjf75FlDX发起攻击, PDET6mWWde受到49点伤害

nXOINTHn5q发起攻击, HnTM6ax71z受到88点伤害

PDET6mWWde发起攻击, X5nHALQsd8回避了攻击

dpCNNufW7B发起攻击, X5nHALQsd8受到39点伤害

emuuGEJFCU开始聚气, emuuGEJFCU攻击力上升

9Qjf75FlDX发起攻击, X5nHALQsd8受到26点伤害

X5nHALQsd8发起攻击, nXOINTHn5q受到109点伤害

GtZHyiydqU发动会心一击, nXOINTHn5q受到83点伤害

PDET6mWWde发起攻击, 9Qjf75FlDX受到63点伤害

nXOINTHn5q发起攻击, dpCNNufW7B受到107点伤害

9Qjf75FlDX发起攻击, dpCNNufW7B受到54点伤害

 dpCNNufW7B被击倒了

HnTM6ax71z发起攻击, PDET6mWWde受到81点伤害

emuuGEJFCU发起攻击, nXOINTHn5q受到51点伤害

PDET6mWWde发起攻击, X5nHALQsd8受到71点伤害

 X5nHALQsd8被击倒了

nXOINTHn5q发起攻击, 9Qjf75FlDX回避了攻击

GtZHyiydqU发起攻击, 9Qjf75FlDX受到46点伤害

9Qjf75FlDX使用魅惑, PDET6mWWde被魅惑了

emuuGEJFCU发起攻击, HnTM6ax71z受到174点伤害

 HnTM6ax71z被击倒了

PDET6mWWde发起攻击, emuuGEJFCU受到95点伤害

 emuuGEJFCU被击倒了

 PDET6mWWde从魅惑中解除

nXOINTHn5q发起攻击, 9Qjf75FlDX受到51点伤害

9Qjf75FlDX发起攻击, GtZHyiydqU受到68点伤害

 GtZHyiydqU被击倒了

PDET6mWWde发起攻击, 9Qjf75FlDX回避了攻击

9Qjf75FlDX发起攻击, PDET6mWWde受到69点伤害

 PDET6mWWde被击倒了

nXOINTHn5q发起攻击, 9Qjf75FlDX受到60点伤害

 9Qjf75FlDX被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-31 must contain a blank separator between input and trace",
        "sampled case-31 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 3974, "large_31 score mismatch");
    assert!(guard < 20_000, "sampled case-31 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-31", &actual_lines, &expected_lines);
}
