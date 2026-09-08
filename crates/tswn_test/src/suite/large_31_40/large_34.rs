use super::*;

pub fn large_34<E: crate::EngineAdapter>() {
    const CASE: &str = r####"rwoiycdN3T
ws9hX5uJwh
CNY0HLRzOx
BPbll4S27a
4l1qq0g27u
fQfZ40hRlV
ugF6VP1ErI
JgkGFCfpzK
gB4u3dlZ8g
JzJUNA3afm


BPbll4S27a发起攻击, gB4u3dlZ8g受到37点伤害

ws9hX5uJwh发起攻击, JzJUNA3afm受到67点伤害

gB4u3dlZ8g发起攻击, rwoiycdN3T受到122点伤害

JzJUNA3afm发起攻击, ws9hX5uJwh受到49点伤害

rwoiycdN3T发起攻击, JzJUNA3afm受到29点伤害

JgkGFCfpzK发动铁壁, JgkGFCfpzK防御力大幅上升

fQfZ40hRlV使用分身, 出现一个新的fQfZ40hRlV

CNY0HLRzOx发起攻击, rwoiycdN3T受到74点伤害

ws9hX5uJwh开始聚气, ws9hX5uJwh攻击力上升

gB4u3dlZ8g使用狂暴术, fQfZ40hRlV受到72点伤害, fQfZ40hRlV进入狂暴状态

ugF6VP1ErI发起攻击, 4l1qq0g27u受到97点伤害

BPbll4S27a发起攻击, JgkGFCfpzK受到1点伤害

rwoiycdN3T发起攻击, fQfZ40hRlV受到24点伤害

ws9hX5uJwh发起攻击, CNY0HLRzOx回避了攻击

4l1qq0g27u发动会心一击, fQfZ40hRlV受到103点伤害, fQfZ40hRlV发动隐匿

fQfZ40hRlV发起攻击, 4l1qq0g27u受到63点伤害

fQfZ40hRlV发起攻击, BPbll4S27a受到38点伤害

CNY0HLRzOx使用分身, 出现一个新的CNY0HLRzOx

JzJUNA3afm发起攻击, gB4u3dlZ8g受到77点伤害

fQfZ40hRlV使用生命之轮, JzJUNA3afm的体力值与fQfZ40hRlV互换

4l1qq0g27u发起攻击, JzJUNA3afm受到49点伤害

 JzJUNA3afm被击倒了

JgkGFCfpzK发起攻击, BPbll4S27a受到108点伤害

ugF6VP1ErI发起攻击, JgkGFCfpzK受到1点伤害

rwoiycdN3T发起攻击, ugF6VP1ErI受到29点伤害

ws9hX5uJwh发起攻击, JgkGFCfpzK受到1点伤害

gB4u3dlZ8g使用火球术, CNY0HLRzOx受到146点伤害

 CNY0HLRzOx被击倒了

CNY0HLRzOx发起攻击, 4l1qq0g27u受到83点伤害

BPbll4S27a发起攻击, JgkGFCfpzK受到1点伤害

JgkGFCfpzK使用净化, ws9hX5uJwh受到19点伤害

 ws9hX5uJwh的聚气被打消了

 JgkGFCfpzK从铁壁中解除

fQfZ40hRlV发起攻击, JgkGFCfpzK受到54点伤害

ugF6VP1ErI发起攻击, BPbll4S27a受到51点伤害

rwoiycdN3T发起攻击, fQfZ40hRlV受到27点伤害

4l1qq0g27u发起攻击, rwoiycdN3T受到44点伤害

ws9hX5uJwh发起攻击, fQfZ40hRlV受到38点伤害

fQfZ40hRlV使用瘟疫, CNY0HLRzOx体力减少47%

BPbll4S27a发起攻击, ws9hX5uJwh受到59点伤害

CNY0HLRzOx发起攻击, gB4u3dlZ8g受到25点伤害

fQfZ40hRlV发起攻击, ugF6VP1ErI受到49点伤害

ws9hX5uJwh发起攻击, fQfZ40hRlV受到89点伤害, fQfZ40hRlV发动隐匿

JgkGFCfpzK发起攻击, BPbll4S27a受到53点伤害

rwoiycdN3T发起攻击, JgkGFCfpzK受到61点伤害

4l1qq0g27u发起攻击, ws9hX5uJwh受到55点伤害

gB4u3dlZ8g使用火球术, fQfZ40hRlV受到123点伤害

 fQfZ40hRlV被击倒了

BPbll4S27a投毒, CNY0HLRzOx受到38点伤害, CNY0HLRzOx中毒

fQfZ40hRlV发起攻击, ugF6VP1ErI受到17点伤害

ugF6VP1ErI发起攻击, 4l1qq0g27u受到32点伤害

rwoiycdN3T发起攻击, ugF6VP1ErI受到75点伤害

CNY0HLRzOx发起攻击, BPbll4S27a回避了攻击

 CNY0HLRzOx毒性发作, CNY0HLRzOx受到42点伤害

 CNY0HLRzOx被击倒了

ws9hX5uJwh发起攻击, ugF6VP1ErI受到66点伤害

JgkGFCfpzK投毒, gB4u3dlZ8g受到62点伤害, gB4u3dlZ8g中毒

4l1qq0g27u发起攻击, ws9hX5uJwh受到130点伤害

 ws9hX5uJwh被击倒了

ugF6VP1ErI发起攻击, rwoiycdN3T受到69点伤害

 rwoiycdN3T被击倒了

BPbll4S27a发起攻击, gB4u3dlZ8g受到94点伤害

gB4u3dlZ8g发起攻击, JgkGFCfpzK受到73点伤害

 gB4u3dlZ8g毒性发作, gB4u3dlZ8g受到26点伤害

4l1qq0g27u发起攻击, BPbll4S27a受到60点伤害

fQfZ40hRlV使用分身, 出现一个新的fQfZ40hRlV

JgkGFCfpzK发起攻击, gB4u3dlZ8g受到56点伤害

 gB4u3dlZ8g被击倒了

ugF6VP1ErI发起攻击, JgkGFCfpzK受到32点伤害

BPbll4S27a发起攻击, 4l1qq0g27u受到83点伤害

 4l1qq0g27u被击倒了

fQfZ40hRlV使用苏生术, fQfZ40hRlV复活了, fQfZ40hRlV回复体力70点

ugF6VP1ErI发起攻击, fQfZ40hRlV受到71点伤害

 fQfZ40hRlV被击倒了

fQfZ40hRlV发起攻击, ugF6VP1ErI受到108点伤害

 ugF6VP1ErI被击倒了

fQfZ40hRlV发起攻击, JgkGFCfpzK受到53点伤害

 JgkGFCfpzK被击倒了

BPbll4S27a发起攻击, fQfZ40hRlV受到65点伤害

 fQfZ40hRlV被击倒了

BPbll4S27a发起攻击, fQfZ40hRlV受到72点伤害

 fQfZ40hRlV被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-34 must contain a blank separator between input and trace",
        "sampled case-34 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 5135, "large_34 score mismatch");
    assert!(guard < 20_000, "sampled case-34 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-34", &actual_lines, &expected_lines);
}
