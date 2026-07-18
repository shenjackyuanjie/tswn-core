use super::*;

#[test]
fn imported_at_boost_keeps_exact_legacy_bits() {
    let template = PlayerTemplate::new(1, "actor", 0, 100, 42).with_at_boost(1.7000000476837158);
    let runtime = PlayerRuntime::from_template(&template, &ExtensionRegistry::default(), EntityIdx(0), EntityIdx(0));

    assert_eq!(template.at_boost_bits, 1.7000000476837158_f64.to_bits());
    assert_eq!(template.at_boost_millionths, 1_700_000);
    assert_eq!(runtime.at_boost().to_bits(), 1.7000000476837158_f64.to_bits());
}

#[test]
fn accumulate_runtime_keeps_float_tail_past_millionths_mirror() {
    let mut arena = EntityArena::from_templates(vec![PlayerTemplate::new(1, "actor", 0, 100, 42)]);

    let actor = arena.get_mut(EntityIdx(0)).unwrap();
    assert!(actor.activate_accumulate_runtime());
    assert_eq!(actor.runtime.at_boost_millionths, 1_700_000);
    assert_eq!(actor.runtime.at_boost().to_bits(), 1.7000000476837158_f64.to_bits());

    let atp = 4200.0 * actor.runtime.at_boost();
    assert!((atp - 7140.000200271606).abs() < f64::EPSILON * 8192.0);
    assert_eq!((atp / 84.0).ceil() as i32, 86);
    assert_eq!((4200.0_f64 * 1.7 / 84.0).ceil() as i32, 85);
}
