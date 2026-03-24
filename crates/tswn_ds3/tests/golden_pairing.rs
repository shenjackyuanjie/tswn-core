use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn project_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

fn fixture_dir(name: &str) -> PathBuf { project_root().join("tests").join("fixtures").join(name) }

fn temp_dir(prefix: &str) -> PathBuf {
    let suffix = SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_nanos()).unwrap_or(0);
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{suffix}", std::process::id()));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn bin_path() -> PathBuf {
    let exe = if cfg!(windows) { "tswn_ds3.exe" } else { "tswn_ds3" };
    PathBuf::from(env!("CARGO_BIN_EXE_tswn_ds3")).parent().expect("binary parent").join(exe)
}

fn run_and_expect_success(args: &[&str]) {
    let output = std::process::Command::new(bin_path()).args(args).output().expect("run tswn_ds3 command");
    assert!(
        output.status.success(),
        "command failed: tswn_ds3 {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn read_bytes(path: &Path) -> Vec<u8> { fs::read(path).unwrap_or_else(|err| panic!("read {} failed: {err}", path.display())) }

fn assert_same_file(actual: &Path, expected: &Path) {
    assert_eq!(
        read_bytes(actual),
        read_bytes(expected),
        "file mismatch:\nactual: {}\nexpected: {}",
        actual.display(),
        expected.display()
    );
}

fn run_pair_mode(mode: &str, pair_type: i32, left: &Path, right: &Path, output: &Path, threads: usize) {
    let pair_type_str = pair_type.to_string();
    let threads_str = threads.to_string();
    run_and_expect_success(&[
        &format!("pair-{mode}"),
        &pair_type_str,
        &threads_str,
        "-100000",
        &left.to_string_lossy(),
        &right.to_string_lossy(),
        &output.to_string_lossy(),
    ]);
}

#[test]
fn golden_pairing_outputs_match_fixture() {
    let fixture = fixture_dir("pairing");
    let input = fixture.join("input");
    let expected = fixture.join("expected");
    let work = temp_dir("tswn-ds3-golden-pairing");

    let fc_out = work.join("fc.txt");
    run_pair_mode("fc", 1, &input.join("left_fz.txt"), &input.join("right_bc.txt"), &fc_out, 4);
    assert_same_file(&fc_out, &expected.join("fc.txt"));

    let wc_out = work.join("wc.txt");
    run_pair_mode("wc", 1, &input.join("left_wc.txt"), &input.join("right_wc.txt"), &wc_out, 4);
    assert_same_file(&wc_out, &expected.join("wc.txt"));

    let rh_out = work.join("rh.txt");
    run_pair_mode("rh", 1, &input.join("left_fs.txt"), &input.join("right_pj.txt"), &rh_out, 4);
    assert_same_file(&rh_out, &expected.join("rh.txt"));

    fs::remove_dir_all(&work).expect("cleanup temp dir");
}

#[test]
fn pairing_threads_1_and_4_are_identical() {
    let fixture = fixture_dir("pairing");
    let input = fixture.join("input");
    let work = temp_dir("tswn-ds3-pair-threads");

    let cases = [
        ("fc", 1, "left_fz.txt", "right_bc.txt"),
        ("wc", 1, "left_wc.txt", "right_wc.txt"),
        ("rh", 1, "left_fs.txt", "right_pj.txt"),
    ];
    for (mode, pair_type, left, right) in cases {
        let one = work.join(format!("{mode}-t1.txt"));
        let many = work.join(format!("{mode}-t4.txt"));
        run_pair_mode(mode, pair_type, &input.join(left), &input.join(right), &one, 1);
        run_pair_mode(mode, pair_type, &input.join(left), &input.join(right), &many, 4);
        assert_same_file(&one, &many);
    }

    fs::remove_dir_all(&work).expect("cleanup temp dir");
}
