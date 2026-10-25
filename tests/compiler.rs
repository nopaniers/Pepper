use std::{ffi::OsStr, fs, path::Path, process::Command};

use pepper::{backend, semantic};

fn program(source: &str) -> semantic::Program {
    semantic::analyze(&pepper::parse(source).unwrap()).unwrap()
}

fn build_source(directory: &Path, source: &str) -> std::process::Output {
    let input = directory.join("source.pepper");
    fs::write(&input, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_pepper"))
        .arg("build")
        .arg(input)
        .arg("-o")
        .arg(directory.join("hello"))
        .output()
        .unwrap()
}

#[test]
fn hello_world_compiles_links_and_runs() {
    let directory = tempfile::tempdir().unwrap();
    let build = build_source(directory.path(), include_str!("../examples/hello.pepper"));
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new(directory.path().join("hello"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"Hello from Pepper!\n");
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_dir(directory.path()).unwrap().count(),
        2,
        "temporary build files leaked"
    );
}

#[test]
fn output_supports_spaces_unicode_percent_signs_and_empty_strings() {
    let directory = tempfile::Builder::new()
        .prefix("pepper path with spaces ")
        .tempdir()
        .unwrap();
    let build = build_source(
        directory.path(),
        "main() =\n  print(\"🌶 Pepper: 100%\\nsecond line\")\n  print(\"\")\n  return\n",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new(directory.path().join("hello"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "🌶 Pepper: 100%\nsecond line\n\n"
    );
}

#[test]
fn a_bare_return_produces_a_successful_silent_executable() {
    let directory = tempfile::tempdir().unwrap();
    let build = build_source(directory.path(), "main() = return");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new(directory.path().join("hello"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn verified_ir_contains_native_entry_point_and_print_call() {
    let ir = backend::emit_ir(&program("main() = print(\"Hello world!\")"), "hello").unwrap();
    assert!(ir.contains("define i32 @main()"));
    assert!(ir.contains("call i32 @puts"));
    assert!(ir.contains("Hello world!"));
    assert!(ir.contains("target triple ="));
}

#[test]
fn executable_subset_rejects_invalid_and_unimplemented_programs() {
    let cases = [
        ("", "missing main"),
        ("main(x: Int) = print(\"hi\")", "must not take parameters"),
        (
            "main() = print(\"hi\")\nmain() = print(\"again\")",
            "duplicate main",
        ),
        ("other() = print(\"hi\")", "only a top-level main"),
        ("main() = print(1)", "String literal"),
        ("main() = print()", "exactly one"),
        ("main() = print(\"a\", \"b\")", "exactly one"),
        ("main() = typo(\"hi\")", "unknown function"),
        ("main() = x.print(\"hi\")", "only calls"),
        ("main() = print(\"a\0b\")", "NUL bytes"),
        ("main() = x = 1", "not supported"),
        ("main() = return 1", "not supported"),
        ("main() = 1 + 2", "expected print"),
        ("main() =\n  return\n  print(\"hi\")", "unreachable"),
    ];
    for (source, expected) in cases {
        let error = semantic::analyze(&pepper::parse(source).unwrap()).unwrap_err();
        assert!(error.message.contains(expected), "{source:?}: {error}");
    }
}

#[test]
fn semantic_failure_is_located_and_does_not_write_executable() {
    let directory = tempfile::tempdir().unwrap();
    let output = build_source(directory.path(), "main() =\n  print(42)\n");
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("source.pepper:2:9: error:"), "{error}");
    assert!(!directory.path().join("hello").exists());
}

#[test]
fn missing_linker_leaves_existing_output_intact_and_cleans_up() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("hello");
    fs::write(&output, b"existing output").unwrap();
    let missing = directory.path().join("missing-linker");
    let error = backend::build_with_linker(
        &program("main() = return"),
        "test",
        &output,
        missing.as_os_str(),
    )
    .unwrap_err();
    assert!(error.contains("cannot run linker"));
    assert_eq!(fs::read(&output).unwrap(), b"existing output");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn failing_linker_leaves_existing_output_intact_and_cleans_up() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("hello");
    fs::write(&output, b"existing output").unwrap();
    let error = backend::build_with_linker(
        &program("main() = return"),
        "test",
        &output,
        OsStr::new("false"),
    )
    .unwrap_err();
    assert!(error.contains("failed"));
    assert_eq!(fs::read(&output).unwrap(), b"existing output");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn cli_rejects_overwriting_source_and_malformed_build_options() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.pepper");
    let source = "main() = return";
    fs::write(&input, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pepper"))
        .arg("build")
        .arg(&input)
        .arg("-o")
        .arg(&input)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must not overwrite"));
    assert_eq!(fs::read_to_string(&input).unwrap(), source);
    for args in [
        vec!["build"],
        vec!["build", "examples/hello.pepper"],
        vec!["build", "examples/hello.pepper", "--typo", "hello"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pepper"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
    }
}
