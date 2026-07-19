use super::*;

pub fn large_24<E: crate::EngineAdapter>() {
    const CASE: &str = r####"JdHjkmcAQX
VJoqJLK130
EcpA1rezSh
bPB0L3QgHn
LVAZaldlZD
s0fJOqwYFq
1kAa6aNXaf
suxiWYFS7n
p1K2MgDJ6F
zp7YuG9eob


zp7YuG9eob发起攻击, 1kAa6aNXaf受到62点伤害

EcpA1rezSh使用净化, VJoqJLK130受到47点伤害

p1K2MgDJ6F使用雷击术

 s0fJOqwYFq受到8点伤害

 s0fJOqwYFq受到15点伤害

 s0fJOqwYFq受到21点伤害

 s0fJOqwYFq回避了攻击

JdHjkmcAQX发起攻击, s0fJOqwYFq受到84点伤害

1kAa6aNXaf发起攻击, zp7YuG9eob受到70点伤害

s0fJOqwYFq发起攻击, suxiWYFS7n受到28点伤害

suxiWYFS7n发起攻击, JdHjkmcAQX受到108点伤害

VJoqJLK130发起攻击, s0fJOqwYFq受到115点伤害

bPB0L3QgHn发起攻击, zp7YuG9eob受到75点伤害

LVAZaldlZD发起攻击, VJoqJLK130受到57点伤害

zp7YuG9eob发起攻击, EcpA1rezSh回避了攻击

suxiWYFS7n发起攻击, VJoqJLK130受到86点伤害

EcpA1rezSh发起攻击, bPB0L3QgHn回避了攻击

1kAa6aNXaf发起攻击, p1K2MgDJ6F受到70点伤害

p1K2MgDJ6F发起吸血攻击, 1kAa6aNXaf受到61点伤害, p1K2MgDJ6F回复体力31点

VJoqJLK130发起攻击, LVAZaldlZD受到55点伤害

LVAZaldlZD使用治愈魔法, LVAZaldlZD回复体力55点

JdHjkmcAQX发起攻击, EcpA1rezSh受到13点伤害

s0fJOqwYFq发起攻击, EcpA1rezSh受到86点伤害

bPB0L3QgHn发起攻击, JdHjkmcAQX受到108点伤害

suxiWYFS7n使用冰冻术, LVAZaldlZD受到44点伤害, LVAZaldlZD被冰冻了

p1K2MgDJ6F发起攻击, bPB0L3QgHn受到74点伤害

EcpA1rezSh发起攻击, bPB0L3QgHn受到79点伤害

zp7YuG9eob发起攻击, 1kAa6aNXaf受到119点伤害

1kAa6aNXaf发起攻击, p1K2MgDJ6F受到54点伤害

bPB0L3QgHn发起攻击, JdHjkmcAQX受到22点伤害

LVAZaldlZD从冰冻中解除

JdHjkmcAQX发起攻击, 1kAa6aNXaf受到66点伤害

 1kAa6aNXaf被击倒了

s0fJOqwYFq发起攻击, p1K2MgDJ6F受到49点伤害

suxiWYFS7n发起攻击, s0fJOqwYFq受到72点伤害

 s0fJOqwYFq被击倒了

VJoqJLK130发起攻击, LVAZaldlZD受到51点伤害

LVAZaldlZD发起攻击, p1K2MgDJ6F受到36点伤害

zp7YuG9eob发起攻击, p1K2MgDJ6F受到54点伤害

EcpA1rezSh发起攻击, bPB0L3QgHn回避了攻击

p1K2MgDJ6F使用分身, 出现一个新的p1K2MgDJ6F

VJoqJLK130发起攻击, suxiWYFS7n受到29点伤害

zp7YuG9eob发起攻击, VJoqJLK130受到68点伤害

JdHjkmcAQX使用诅咒, VJoqJLK130受到132点伤害

 VJoqJLK130被击倒了

p1K2MgDJ6F发起攻击, LVAZaldlZD受到55点伤害

EcpA1rezSh发起攻击, bPB0L3QgHn受到73点伤害

bPB0L3QgHn发起攻击, suxiWYFS7n回避了攻击

LVAZaldlZD使用治愈魔法, LVAZaldlZD回复体力97点

suxiWYFS7n发起攻击, LVAZaldlZD受到42点伤害

JdHjkmcAQX发起攻击, zp7YuG9eob受到58点伤害

p1K2MgDJ6F使用雷击术

 zp7YuG9eob受到19点伤害

 zp7YuG9eob受到18点伤害

 zp7YuG9eob受到4点伤害

 zp7YuG9eob受到9点伤害

bPB0L3QgHn发起攻击, JdHjkmcAQX受到24点伤害

EcpA1rezSh发起攻击, p1K2MgDJ6F受到50点伤害

 p1K2MgDJ6F被击倒了

LVAZaldlZD使用诅咒, JdHjkmcAQX受到79点伤害

 JdHjkmcAQX被击倒了

zp7YuG9eob投毒, EcpA1rezSh受到77点伤害, EcpA1rezSh中毒

p1K2MgDJ6F发起攻击, suxiWYFS7n受到21点伤害

suxiWYFS7n发起攻击, EcpA1rezSh受到73点伤害

EcpA1rezSh使用净化, suxiWYFS7n受到37点伤害

 EcpA1rezSh毒性发作, EcpA1rezSh受到17点伤害

p1K2MgDJ6F使用火球术, zp7YuG9eob受到142点伤害

 zp7YuG9eob被击倒了

suxiWYFS7n发起攻击, LVAZaldlZD受到60点伤害

EcpA1rezSh发起攻击, LVAZaldlZD受到35点伤害

 EcpA1rezSh毒性发作, EcpA1rezSh受到14点伤害

 EcpA1rezSh被击倒了

bPB0L3QgHn发起攻击, LVAZaldlZD受到58点伤害

LVAZaldlZD发起攻击, suxiWYFS7n受到78点伤害

p1K2MgDJ6F使用雷击术

 suxiWYFS7n回避了攻击

suxiWYFS7n使用冰冻术, LVAZaldlZD受到64点伤害

 LVAZaldlZD被击倒了

bPB0L3QgHn发起攻击, suxiWYFS7n受到103点伤害

 suxiWYFS7n被击倒了

p1K2MgDJ6F发起攻击, bPB0L3QgHn受到24点伤害

 bPB0L3QgHn被击倒了, bPB0L3QgHn使用护身符抵挡了一次死亡, bPB0L3QgHn回复体力3点

bPB0L3QgHn发起攻击, p1K2MgDJ6F受到62点伤害

 p1K2MgDJ6F被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-24 must contain a blank separator between input and trace",
        "sampled case-24 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 4517, "large_24 score mismatch");

    assert!(guard < 20_000, "sampled case-24 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-24", &actual_lines, &expected_lines);
}
