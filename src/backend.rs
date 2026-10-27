//! Native LLVM code generation via Inkwell, linked with a Rust runtime by rustc.
use std::{env, ffi::OsStr, fs, path::Path, process::Command};

use inkwell::{
    OptimizationLevel,
    context::Context,
    targets::{CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine},
};

use crate::{codegen::generate, semantic::Program};

fn target_machine() -> Result<TargetMachine, String> {
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| format!("cannot initialize LLVM's native target: {e}"))?;
    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple).map_err(|e| e.to_string())?;
    target
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::None,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .ok_or_else(|| "LLVM could not create a native target machine".into())
}

/// Produce verified LLVM IR for inspection. This does not invoke a linker.
pub fn emit_ir(program: &Program, name: &str) -> Result<String, String> {
    let context = Context::create();
    let machine = target_machine()?;
    let module = generate(&context, program, name, &machine)?;
    Ok(module.print_to_string().to_string())
}

/// Emit a native LLVM object, compile the bundled Rust runtime crate, then use
/// rustc to link the runtime, Rust startup and Pepper object into an executable.
/// RUSTC selects the Rust compiler executable (not a shell command).
/// Temporary files are cleaned up and failed builds preserve an existing output.
pub fn build(program: &Program, name: &str, output: &Path) -> Result<(), String> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    build_with_rustc(program, name, output, &rustc)
}

pub fn build_with_rustc(
    program: &Program,
    name: &str,
    output: &Path,
    rustc: &OsStr,
) -> Result<(), String> {
    let output = env::current_dir().map_err(|e| e.to_string())?.join(output);
    let parent = output
        .parent()
        .ok_or("output path must have a parent directory")?;
    let temporary = tempfile::Builder::new()
        .prefix(".pepper-")
        .tempdir_in(parent)
        .map_err(|e| format!("cannot create build directory in {}: {e}", parent.display()))?;
    let object = temporary.path().join("program.o");
    let executable = temporary.path().join("program");
    let runtime_source = temporary.path().join("runtime.rs");
    let runtime_library = temporary.path().join("libpepper_runtime.rlib");
    let host_source = temporary.path().join("host.rs");
    fs::write(&runtime_source, include_str!("../runtime/src/lib.rs"))
        .map_err(|e| format!("cannot write Rust runtime: {e}"))?;
    fs::write(&host_source, include_str!("../runtime/host.rs"))
        .map_err(|e| format!("cannot write Rust startup: {e}"))?;
    let context = Context::create();
    let machine = target_machine()?;
    let module = generate(&context, program, name, &machine)?;
    machine
        .write_to_file(&module, FileType::Object, &object)
        .map_err(|e| format!("LLVM object emission failed: {e}"))?;

    let mut runtime = Command::new(rustc);
    runtime
        .args([
            "--edition=2024",
            "--crate-name=pepper_runtime",
            "--crate-type=rlib",
        ])
        .arg(&runtime_source)
        .arg("-o")
        .arg(&runtime_library);
    run_rustc(&mut runtime, "Rust runtime compilation")?;

    let mut dependency = std::ffi::OsString::from("pepper_runtime=");
    dependency.push(&runtime_library);
    let mut link_argument = std::ffi::OsString::from("link-arg=");
    link_argument.push(&object);
    let mut link = Command::new(rustc);
    link.args(["--edition=2024", "--crate-name=pepper_program"])
        .arg(&host_source)
        .arg("--extern")
        .arg(dependency)
        .arg("-C")
        .arg(link_argument)
        .arg("-o")
        .arg(&executable);
    run_rustc(&mut link, "Rust executable linking")?;
    fs::rename(&executable, &output)
        .map_err(|e| format!("cannot write executable {}: {e}", output.display()))?;
    Ok(())
}

fn run_rustc(command: &mut Command, stage: &str) -> Result<(), String> {
    let result = command.output().map_err(|e| {
        format!(
            "cannot run Rust compiler {}: {e}",
            command.get_program().to_string_lossy()
        )
    })?;
    if !result.status.success() {
        return Err(format!(
            "{stage} failed ({}):\n{}{}",
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(())
}
