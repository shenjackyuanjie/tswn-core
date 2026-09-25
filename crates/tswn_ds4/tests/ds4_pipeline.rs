use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(root: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_tswn_ds4"))
        .args(["run", "--root", root.to_str().expect("root path")])
        .output()
        .expect("start tswn_ds4");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("tswn-ds4-pipeline-json-{}", std::process::id()));
    fs::create_dir_all(root.join("input")).expect("create input dir");
    root
}

#[test]
fn json_pipeline_filters_team_and_preserves_incremental_state() {
    let root = temp_root();
    fs::write(
        root.join("input/names.txt"),
        "alpha@teamA\nbeta@teamA\ngamma@teamA\nother@teamB\ninvalid\n",
    )
    .expect("write input");
    fs::write(
        root.join("config.json"),
        r#"{
            "team_name":"teamA", "thread_number":2,
            "bc":{"sieve":-100000,"ptt_sieve":-100000},
            "fz":{"sieve":-100000,"ptt_sieve":-100000},
            "wc":{"sieve":-100000,"ptt_sieve":-100000},
            "fs":{"sieve":-100000,"ptt_sieve":-100000},
            "pj":{"sieve":-100000,"ptt_sieve":-100000},
            "two_fc":{"enable":1,"sieve":-100000},
            "two_wc":{"enable":1,"sieve":-100000},
            "two_rh":{"enable":1,"sieve":-100000},
            "qp":{"enable":1,"sieve":-100000,"skill_sieve":300},
            "qd":{"enable":1,"sieve":-100000,"skill_sieve":300},
            "pp":{"enable":1,"sieve":-100000,"skill_sieve":300},
            "pd":{"enable":1,"sieve":-100000},
            "cqd":{"enable":1,"sieve":-100000},
            "copy_pf_to_out":1, "copy_to_new":1, "run_dup":1,
            "abcp":{"enable":0,"sieve":4500}, "get_3":0
        }"#,
    )
    .expect("write config");

    run(&root);
    let ignored = fs::read_to_string(root.join("out/ignore_input.txt")).expect("ignored rows");
    assert!(ignored.contains("other@teamB"));
    assert!(ignored.contains("invalid"));
    let old = fs::read_to_string(root.join("file/old.txt")).expect("old names");
    assert_eq!(old.lines().count(), 3);
    assert!(root.join("out/new_qp.txt").exists());
    assert!(root.join("file/old_qp.txt").exists());
    let fc = fs::read_to_string(root.join("out/FC.txt")).expect("FC pairs");
    assert!(fc.lines().count() >= 6);
    assert!(fc.contains("+beta@teamA"));
    assert!(fc.contains("+gamma@teamA"));

    run(&root);
    assert_eq!(
        fs::read_to_string(root.join("file/old.txt")).expect("old names").lines().count(),
        3
    );
    assert!(fs::read_to_string(root.join("out/FC.txt")).expect("FC pairs after rerun").is_empty());
    fs::remove_dir_all(root).expect("cleanup temp dir");
}
