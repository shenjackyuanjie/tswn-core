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
    let actual_bytes = read_bytes(actual);
    let expected_bytes = read_bytes(expected);
    assert_eq!(
        actual_bytes,
        expected_bytes,
        "file mismatch:\nactual: {}\nexpected: {}",
        actual.display(),
        expected.display()
    );
}

fn run_single_mode(mode: &str, input: &Path, output: &Path, threads: usize) {
    let threads_str = threads.to_string();
    run_and_expect_success(&[
        &format!("score-{mode}"),
        &threads_str,
        &input.to_string_lossy(),
        &output.to_string_lossy(),
        "-100000",
        "-100000",
    ]);
}

#[test]
fn golden_single_outputs_match_fixture() {
    let fixture = fixture_dir("single");
    let work = temp_dir("tswn-ds3-golden-single");
    let input = fixture.join("input").join("names.txt");

    for mode in ["bc", "fz", "wc", "fs", "pj"] {
        let actual = work.join(format!("{mode}.txt"));
        let expected = fixture.join("expected").join(format!("{mode}.txt"));
        run_single_mode(mode, &input, &actual, 4);
        assert_same_file(&actual, &expected);
    }

    fs::remove_dir_all(&work).expect("cleanup temp dir");
}

#[test]
fn single_threads_1_and_4_are_identical() {
    let fixture = fixture_dir("single");
    let work = temp_dir("tswn-ds3-single-threads");
    let input = fixture.join("input").join("names.txt");

    for mode in ["bc", "fz", "wc", "fs", "pj"] {
        let one = work.join(format!("{mode}-t1.txt"));
        let many = work.join(format!("{mode}-t4.txt"));
        run_single_mode(mode, &input, &one, 1);
        run_single_mode(mode, &input, &many, 4);
        assert_same_file(&one, &many);
    }

    fs::remove_dir_all(&work).expect("cleanup temp dir");
}
