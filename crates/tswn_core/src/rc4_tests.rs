use super::*;

fn reference_key_scheduling(mut state: [u8; VAL_LEN], keys: &[u8], rounds: usize) -> [u8; VAL_LEN] {
    let key_len = keys.len();
    for _ in 0..rounds {
        let mut j = 0usize;
        for x in 0..VAL_LEN {
            j = (j + state[x] as usize + keys[x % key_len] as usize) & 255;
            state.swap(x, j);
        }
    }
    state
}

fn reference_next_u8(state: &mut [u8; VAL_LEN], i: &mut u32, j: &mut u32) -> u8 {
    *i = i.wrapping_add(1) & 255;
    *j = j.wrapping_add(state[*i as usize] as u32) & 255;
    state.swap(*i as usize, *j as usize);
    state[(state[*i as usize] as usize + state[*j as usize] as usize) & 255]
}

#[test]
fn unchecked_prga_matches_safe_reference() {
    for key_len in [1usize, 2, 7, 16, 31, 255, 256] {
        let keys = (0..key_len)
            .map(|index| (index as u8).wrapping_mul(73).wrapping_add(41))
            .collect::<Vec<_>>();
        let mut actual = RC4::new(&keys, 2);
        let mut expected_state = actual.main_val;
        let mut expected_i = 0u32;
        let mut expected_j = 0u32;
        for step in 0..4096 {
            let expected = reference_next_u8(&mut expected_state, &mut expected_i, &mut expected_j);
            assert_eq!(actual.next_u8(), expected, "key_len={key_len}, step={step}");
        }
        assert_eq!(actual.main_val, expected_state, "key_len={key_len}");
        assert_eq!((actual.i, actual.j), (expected_i, expected_j), "key_len={key_len}");
    }
}

#[test]
fn peek_matches_next_byte_without_changing_state() {
    let mut rng = RC4::new(b"peek-regression", 2);
    for step in 0..4096 {
        let before = rng.clone();
        let mut expected = before.clone();
        let next = expected.next_u8();
        assert_eq!(rng.peek_next_u8(), next, "step={step}");
        assert_eq!(rng.peek_next_u8(), next, "重复观察：step={step}");
        assert_eq!((rng.i, rng.j), (before.i, before.j));
        assert_eq!(rng.main_val, before.main_val);
        #[cfg(not(feature = "no_debug"))]
        assert_eq!(rng.byte_count, before.byte_count);
        assert_eq!(rng.next_u8(), next);
    }
}

#[test]
fn peek_handles_output_collisions_and_wrapped_cursors() {
    for (case, i, j, swap_zero_one) in [
        ("输出下标与 i 相撞", 0, 255, false),
        ("输出下标与 j 相撞", 0, 2, true),
        ("i 与 j 相同", 0, 0, false),
        ("输出下标不相撞", 0, 3, false),
        ("游标只使用低八位", u32::MAX, u32::MAX, false),
    ] {
        let mut rng = RC4::default();
        rng.i = i;
        rng.j = j;
        if swap_zero_one {
            rng.main_val.swap(0, 1);
        }
        let before = rng.clone();
        let mut expected = before.clone();
        let next = reference_next_u8(&mut expected.main_val, &mut expected.i, &mut expected.j);
        assert_eq!(rng.peek_next_u8(), next, "{case}");
        assert_eq!(rng.peek_next_u8(), next, "重复观察：{case}");
        assert_eq!((rng.i, rng.j), (before.i, before.j), "{case}");
        assert_eq!(rng.main_val, before.main_val, "{case}");
        #[cfg(not(feature = "no_debug"))]
        assert_eq!(rng.byte_count, before.byte_count, "{case}");
        assert_eq!(rng.next_u8(), next, "{case}");
    }
}

#[test]
fn bounded_integer_sampling_preserves_compatibility_byte_counts() {
    for (max, consumed) in [(0, 0), (1, 2), (255, 2), (256, 3), (65535, 3), (65536, 4), (0x0080_0000, 4)] {
        let mut actual = RC4::new(b"integer-boundaries", 1);
        let mut expected = actual.clone();
        for _ in 0..consumed {
            reference_next_u8(&mut expected.main_val, &mut expected.i, &mut expected.j);
        }
        let value = actual.next_i32(max);
        if max == 0 {
            assert_eq!(value, 0);
        } else {
            assert!((0..max).contains(&value), "max={max}, value={value}");
        }
        assert_eq!((actual.i, actual.j), (expected.i, expected.j), "max={max}");
        assert_eq!(actual.main_val, expected.main_val, "max={max}");
    }
}

#[test]
fn degenerate_selection_does_not_consume_random_bytes() {
    let mut actual = RC4::new(b"selection-boundaries", 1);
    let before = actual.clone();
    assert_eq!(actual.pick::<u8>(&[]), None);
    assert_eq!(actual.pick(&[7]), Some(0));
    assert_eq!(actual.pick_skip::<u8>(&[], 0), None);
    assert_eq!(actual.pick_skip(&[7], 0), None);
    assert_eq!(actual.pick_skip(&[7], 1), Some(0));
    assert_eq!(actual.pick_skip_range(&[7], &[0]), None);
    assert_eq!(actual.pick_skip_range(&[7], &[]), Some(0));
    assert_eq!(actual.pick_skip_range(&[7, 8], &[0, 1]), None);
    assert_eq!((actual.i, actual.j), (before.i, before.j));
    assert_eq!(actual.main_val, before.main_val);

    let mut expected = before;
    expected.next_i32(1);
    assert_eq!(actual.pick_skip(&[7, 8], 0), Some(1));
    assert_eq!((actual.i, actual.j), (expected.i, expected.j));
    assert_eq!(actual.main_val, expected.main_val);
}

#[test]
fn optimized_key_scheduling_matches_safe_reference() {
    for key_len in [1usize, 2, 3, 7, 16, 31, 255, 256] {
        let keys: Vec<u8> = (0..key_len).map(|index| (index as u8).wrapping_mul(181).wrapping_add(160)).collect();

        for rounds in [1usize, 2, 3] {
            let expected = reference_key_scheduling(VAL_INIT, &keys, rounds);
            assert_eq!(RC4::new(&keys, rounds).main_val, expected, "key_len={key_len}, rounds={rounds}");

            let prefix = [0, 2, 97, 98, 99];
            let initial = RC4::new(&prefix, 1).main_val;
            let expected = reference_key_scheduling(initial, &keys, rounds);

            let mut updated = RC4::new(&prefix, 1);
            updated.update(&keys, rounds);
            assert_eq!(updated.main_val, expected, "update: key_len={key_len}, rounds={rounds}");

            let mut rounded = RC4::new(&prefix, 1);
            rounded.i = 37;
            rounded.j = 91;
            rounded.round(&keys, Some(rounds));
            assert_eq!(rounded.main_val, expected, "round: key_len={key_len}, rounds={rounds}");
            assert_eq!((rounded.i, rounded.j), (0, 0));
        }
    }
}

#[test]
fn cached_key_schedule_prefix_matches_full_schedule() {
    for key_len in [1usize, 2, 7, 31, 255, 256, 300] {
        let keys = (0..key_len)
            .map(|index| (index as u8).wrapping_mul(73).wrapping_add(19))
            .collect::<Vec<_>>();
        for prefix_len in [1usize, key_len.min(7), key_len.min(VAL_LEN)] {
            let prefix = Rc4KeySchedulePrefix::new(&keys[..prefix_len]);
            let actual = RC4::new_with_key_schedule_prefix(&keys, &prefix).expect("相同前缀应命中检查点");
            let expected = RC4::new(&keys, 1);
            assert_eq!(actual.main_val, expected.main_val, "key_len={key_len}, prefix_len={prefix_len}");
        }
    }

    let prefix = Rc4KeySchedulePrefix::new(b"seed:");
    assert!(RC4::new_with_key_schedule_prefix(b"other-key", &prefix).is_none());
}

#[test]
fn interleaved_key_scheduling_matches_independent_updates() {
    for same_len in [false, true] {
        for width in 1..=5 {
            let keys = (0..width)
                .map(|lane| {
                    let key_len = if same_len { 7 } else { lane + 2 };
                    (0..key_len)
                        .map(|index| (lane as u8).wrapping_mul(73).wrapping_add(index as u8).wrapping_add(1))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut expected = (0..width).map(|lane| RC4::new(&[0, lane as u8 + 1], 1)).collect::<Vec<_>>();
            let mut actual = expected.clone();
            for (state, key) in expected.iter_mut().zip(&keys) {
                state.update(key, 2);
            }
            let key_refs = keys.iter().map(Vec::as_slice).collect::<Vec<_>>();
            RC4::update_interleaved(&mut actual, &key_refs, 2);
            for lane in 0..width {
                assert_eq!(
                    actual[lane].main_val, expected[lane].main_val,
                    "same_len={same_len}, width={width}, lane={lane}"
                );
            }
        }
    }
}

#[test]
fn rc4_sort_int_test() {
    let input: [u8; 12] = [97, 97, 97, 97, 97, 97, 13, 98, 98, 98, 98, 98];

    let mut rc4 = RC4::new(&input, 1);

    let result: [u8; 256] = [
        108, 163, 77, 153, 146, 85, 39, 164, 152, 55, 18, 211, 206, 228, 151, 57, 35, 145, 51, 110, 30, 150, 90, 134, 219, 41,
        244, 94, 226, 189, 129, 144, 132, 66, 15, 170, 21, 112, 140, 198, 143, 14, 79, 95, 34, 83, 58, 98, 64, 113, 117, 60, 50,
        103, 17, 67, 43, 162, 234, 8, 207, 190, 154, 40, 53, 212, 209, 188, 109, 33, 175, 88, 195, 171, 86, 31, 74, 5, 193, 80,
        182, 105, 96, 159, 160, 202, 148, 9, 48, 102, 135, 116, 251, 54, 106, 27, 131, 62, 26, 184, 173, 99, 215, 20, 19, 93, 47,
        89, 243, 29, 168, 227, 221, 194, 36, 133, 81, 7, 52, 217, 213, 139, 253, 235, 42, 119, 46, 185, 222, 179, 203, 165, 68,
        245, 176, 128, 246, 124, 240, 130, 174, 142, 157, 6, 38, 28, 177, 248, 237, 205, 180, 201, 59, 138, 24, 252, 249, 232,
        230, 216, 141, 183, 155, 72, 69, 12, 238, 208, 199, 10, 75, 76, 181, 125, 122, 204, 247, 87, 118, 220, 149, 70, 218, 61,
        186, 200, 1, 224, 254, 82, 91, 84, 107, 223, 0, 115, 63, 44, 104, 3, 111, 121, 161, 192, 71, 56, 236, 2, 225, 97, 214,
        127, 210, 178, 137, 13, 158, 100, 250, 242, 241, 23, 187, 25, 169, 126, 11, 114, 22, 65, 229, 233, 231, 197, 196, 123,
        167, 73, 156, 78, 147, 172, 255, 120, 37, 45, 101, 92, 191, 239, 166, 16, 49, 32, 136, 4,
    ];

    assert_eq!(rc4.main_val, result);

    rc4.js_xor_bytes(input.clone().as_mut());

    let result: [u8; 256] = [
        108, 72, 18, 189, 69, 99, 20, 81, 34, 104, 56, 67, 190, 228, 151, 57, 35, 145, 51, 110, 30, 150, 90, 134, 219, 41, 244,
        94, 226, 153, 129, 144, 132, 66, 15, 170, 21, 112, 140, 198, 143, 14, 79, 95, 152, 83, 58, 98, 64, 113, 117, 60, 50, 103,
        17, 211, 43, 162, 234, 8, 207, 206, 154, 40, 53, 212, 209, 188, 109, 33, 175, 88, 195, 171, 86, 31, 74, 5, 193, 80, 182,
        105, 96, 159, 160, 202, 148, 9, 48, 102, 135, 116, 251, 54, 106, 27, 131, 62, 26, 184, 173, 85, 215, 39, 19, 93, 47, 89,
        243, 29, 168, 227, 221, 194, 36, 133, 164, 7, 52, 217, 213, 139, 253, 235, 42, 119, 46, 185, 222, 179, 203, 165, 68, 245,
        176, 128, 246, 124, 240, 130, 174, 142, 157, 6, 38, 28, 177, 248, 237, 205, 180, 201, 59, 138, 24, 252, 249, 232, 230,
        216, 141, 183, 155, 163, 146, 12, 238, 208, 199, 10, 75, 76, 181, 125, 122, 204, 247, 87, 118, 220, 149, 70, 218, 61,
        186, 200, 1, 224, 254, 82, 91, 84, 107, 223, 0, 115, 63, 44, 55, 3, 111, 121, 161, 192, 71, 77, 236, 2, 225, 97, 214,
        127, 210, 178, 137, 13, 158, 100, 250, 242, 241, 23, 187, 25, 169, 126, 11, 114, 22, 65, 229, 233, 231, 197, 196, 123,
        167, 73, 156, 78, 147, 172, 255, 120, 37, 45, 101, 92, 191, 239, 166, 16, 49, 32, 136, 4,
    ];

    assert_eq!(rc4.main_val, result);
}
