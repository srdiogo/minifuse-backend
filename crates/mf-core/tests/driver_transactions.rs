// SPDX-License-Identifier: MIT
//! Run the production C transaction header against in-memory kernel/USB hooks.
#[test]
fn model_driver_transactions_handle_rejection_partial_writes_and_lost_replies() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("mf-driver-faults-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let executable = directory.join("driver-transactions");
    let compiled = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-O2",
            // Match the kernel's packed-record and signed-comparison warning policy.
            // Runtime alignment/bounds checks remain enabled by the sanitizers.
            "-Wno-address-of-packed-member",
            "-Wno-sign-compare",
            "-fsanitize=address,undefined",
            "-fno-omit-frame-pointer",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/driver_transactions.c"),
            "-o",
        ])
        .arg(&executable)
        .output()
        .expect("C compiler required for driver fault injection");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let result = std::process::Command::new(&executable).output().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let output = String::from_utf8(result.stdout).unwrap();
    assert!(output.contains("no hardware access"));
    print!("{output}");
}
