use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let goldens = tswn_test::golden::freeze_legacy_stress_goldens();
    let mut output = serde_json::to_string_pretty(&goldens)?;
    output.push('\n');
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cases/runtime_stress/golden.json");
    fs::write(&path, output)?;
    println!("wrote {} frozen stress goldens to {}", goldens.cases.len(), path.display());
    Ok(())
}
