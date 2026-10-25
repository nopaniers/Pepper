use std::process::Command;

#[test]
fn check_and_parse_examples() {
    let check = Command::new(env!("CARGO_BIN_EXE_pepper"))
        .args(["check", "examples/hello.pepper"])
        .output()
        .unwrap();
    assert!(check.status.success());
    assert!(String::from_utf8_lossy(&check.stdout).contains("syntax OK"));
    let parse = Command::new(env!("CARGO_BIN_EXE_pepper"))
        .args(["parse", "examples/functions.pepper"])
        .output()
        .unwrap();
    assert!(parse.status.success());
    assert!(String::from_utf8_lossy(&parse.stdout).contains("Function"));
}

#[test]
fn missing_files_and_invalid_usage_fail() {
    for args in [
        vec!["check", "examples/does-not-exist.pepper"],
        vec!["run"],
        vec![],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pepper"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
    }
}
