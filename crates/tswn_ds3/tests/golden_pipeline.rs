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

fn copy_dir_contents(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create dst");
    for entry in fs::read_dir(src).expect("read src dir") {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_contents(&path, &target);
        } else {
            fs::copy(&path, &target)
                .unwrap_or_else(|err| panic!("copy {} -> {} failed: {err}", path.display(), target.display()));
        }
    }
}

fn bin_path() -> PathBuf {
    let exe = if cfg!(windows) { "tswn_ds3.exe" } else { "tswn_ds3" };
    PathBuf::from(env!("CARGO_BIN_EXE_tswn_ds3")).parent().expect("binary parent").join(exe)
}

fn run_pipeline(root: &Path) {
    let output = std::process::Command::new(bin_path())
        .args(["run", "--root", &root.to_string_lossy()])
        .output()
        .expect("run tswn_ds3 run");
    assert!(
        output.status.success(),
        "pipeline run failed at {}\nstdout:\n{}\nstderr:\n{}",
        root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_same_file(actual: &Path, expected: &Path) {
    let actual_bytes = fs::read(actual).unwrap_or_else(|err| panic!("read {} failed: {err}", actual.display()));
    let expected_bytes = fs::read(expected).unwrap_or_else(|err| panic!("read {} failed: {err}", expected.display()));
    assert_eq!(
        actual_bytes,
        expected_bytes,
        "file mismatch:\nactual: {}\nexpected: {}",
        actual.display(),
        expected.display()
    );
}

fn assert_expected_tree(work_root: &Path, expected_root: &Path, validate_dirs: &[&str]) {
    for dir in validate_dirs {
        let actual_dir = work_root.join(dir);
        let expected_dir = expected_root.join(dir);
        assert!(expected_dir.exists(), "missing expected dir: {}", expected_dir.display());
        assert!(actual_dir.exists(), "missing actual dir: {}", actual_dir.display());

        let mut stack = vec![expected_dir.clone()];
        while let Some(cur) = stack.pop() {
            for entry in fs::read_dir(&cur).expect("read expected tree") {
                let entry = entry.expect("entry");
                let path = entry.path();
                let rel = path.strip_prefix(&expected_dir).expect("relative expected path");
                let actual_path = actual_dir.join(rel);
                if path.is_dir() {
                    stack.push(path);
                } else {
                    assert!(actual_path.exists(), "missing actual file {}", actual_path.display());
                    assert_same_file(&actual_path, &path);
                }
            }
        }
    }
}

fn prepare_fixture_work(fixture_name: &str) -> (PathBuf, PathBuf) {
    let fixture = fixture_dir(fixture_name);
    let expected = fixture.join("expected");
    let work = temp_dir(&format!("tswn-ds3-pipeline-{fixture_name}"));
    copy_dir_contents(&fixture, &work);
    let expected_in_work = work.join("expected");
    if expected_in_work.exists() {
        fs::remove_dir_all(&expected_in_work).expect("remove copied expected");
    }
    (work, expected)
}

#[test]
fn golden_pipeline_basic_matches_expected() {
    let (work, expected) = prepare_fixture_work("basic");
    run_pipeline(&work);
    assert_expected_tree(&work, &expected, &["tmp", "file", "new", "out"]);
    fs::remove_dir_all(&work).expect("cleanup temp dir");
}

#[test]
fn golden_pipeline_no_dedup_no_copy_matches_expected() {
    let (work, expected) = prepare_fixture_work("no_dedup_no_copy");
    run_pipeline(&work);
    assert_expected_tree(&work, &expected, &["tmp", "file"]);
    let out_files = fs::read_dir(work.join("out"))
        .expect("read out dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .count();
    let new_files = fs::read_dir(work.join("new"))
        .expect("read new dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .count();
    assert_eq!(out_files, 0, "no_dedup_no_copy should not emit pair output files");
    assert_eq!(new_files, 0, "no_dedup_no_copy should not emit new/* files");
    fs::remove_dir_all(&work).expect("cleanup temp dir");
}

#[test]
fn pipeline_outputs_stable_between_threads_1_and_4() {
    let (work1, expected) = prepare_fixture_work("basic");
    let (work4, _) = prepare_fixture_work("basic");

    let cfg1 = "1\r\n-100000 -100000\r\n-100000 -100000\r\n-100000 -100000\r\n-100000 -100000\r\n-100000 -100000\r\n1 -100000\r\n1 -100000\r\n1 -100000\r\n1 1\r\n";
    let cfg4 = "4\r\n-100000 -100000\r\n-100000 -100000\r\n-100000 -100000\r\n-100000 -100000\r\n-100000 -100000\r\n1 -100000\r\n1 -100000\r\n1 -100000\r\n1 1\r\n";
    fs::write(work1.join("config.txt"), cfg1).expect("write cfg1");
    fs::write(work4.join("config.txt"), cfg4).expect("write cfg4");

    run_pipeline(&work1);
    run_pipeline(&work4);

    assert_expected_tree(&work1, &expected, &["tmp", "file", "new", "out"]);
    assert_expected_tree(&work4, &expected, &["tmp", "file", "new", "out"]);
    for dir in ["tmp", "file", "new", "out"] {
        let expected_dir = expected.join(dir);
        let mut stack = vec![expected_dir.clone()];
        while let Some(cur) = stack.pop() {
            for entry in fs::read_dir(&cur).expect("read expected for compare") {
                let entry = entry.expect("entry");
                let path = entry.path();
                let rel = path.strip_prefix(&expected_dir).expect("relative expected path");
                if path.is_dir() {
                    stack.push(path);
                } else {
                    assert_same_file(&work1.join(dir).join(rel), &work4.join(dir).join(rel));
                }
            }
        }
    }

    fs::remove_dir_all(&work1).expect("cleanup temp dir");
    fs::remove_dir_all(&work4).expect("cleanup temp dir");
}
