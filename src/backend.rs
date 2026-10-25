//! Native LLVM code generation via Inkwell, followed by system linking.
use std::{env, ffi::OsStr, fs, path::Path, process::Command};

use inkwell::{
    AddressSpace, OptimizationLevel,
    context::Context,
    module::Module,
    targets::{CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine},
};

use crate::semantic::{Instruction, Program};

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

fn generate<'ctx>(
    context: &'ctx Context,
    program: &Program,
    name: &str,
    machine: &TargetMachine,
) -> Result<Module<'ctx>, String> {
    let module = context.create_module(name);
    module.set_triple(&machine.get_triple());
    module.set_data_layout(&machine.get_target_data().get_data_layout());
    let builder = context.create_builder();
    let i32_type = context.i32_type();
    let pointer_type = context.ptr_type(AddressSpace::default());
    let puts = module.add_function(
        "puts",
        i32_type.fn_type(&[pointer_type.into()], false),
        None,
    );
    let main = module.add_function("main", i32_type.fn_type(&[], false), None);
    let entry = context.append_basic_block(main, "entry");
    builder.position_at_end(entry);
    let mut returned = false;
    for instruction in &program.instructions {
        if returned {
            return Err("invalid lowered program: instruction after return".into());
        }
        match instruction {
            Instruction::Print(text) => {
                if text.contains('\0') {
                    return Err("NUL bytes in printed strings are not supported yet".into());
                }
                let string = builder
                    .build_global_string_ptr(text, "str")
                    .map_err(|e| e.to_string())?;
                builder
                    .build_call(puts, &[string.as_pointer_value().into()], "print_result")
                    .map_err(|e| e.to_string())?;
            }
            Instruction::Return => {
                builder
                    .build_return(Some(&i32_type.const_zero()))
                    .map_err(|e| e.to_string())?;
                returned = true;
            }
        }
    }
    if !returned {
        builder
            .build_return(Some(&i32_type.const_zero()))
            .map_err(|e| e.to_string())?;
    }
    module
        .verify()
        .map_err(|e| format!("LLVM verification failed: {e}"))?;
    Ok(module)
}

/// Produce verified LLVM IR for inspection. This does not invoke a linker.
pub fn emit_ir(program: &Program, name: &str) -> Result<String, String> {
    let context = Context::create();
    let machine = target_machine()?;
    let module = generate(&context, program, name, &machine)?;
    Ok(module.print_to_string().to_string())
}

/// Emit a native object file using LLVM, then link a standalone executable.
///
/// The linker driver defaults to `cc`; CC can select a different executable path
/// (not a shell command). Temporary files live beside the requested output and are
/// cleaned up on success or failure. A failed link leaves an existing output intact.
pub fn build(program: &Program, name: &str, output: &Path) -> Result<(), String> {
    let linker = env::var_os("CC").unwrap_or_else(|| "cc".into());
    build_with_linker(program, name, output, &linker)
}

pub fn build_with_linker(
    program: &Program,
    name: &str,
    output: &Path,
    linker: &OsStr,
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
    let context = Context::create();
    let machine = target_machine()?;
    let module = generate(&context, program, name, &machine)?;
    machine
        .write_to_file(&module, FileType::Object, &object)
        .map_err(|e| format!("LLVM object emission failed: {e}"))?;
    let result = Command::new(linker)
        .arg(&object)
        .arg("-o")
        .arg(&executable)
        .output()
        .map_err(|e| format!("cannot run linker driver {}: {e}", linker.to_string_lossy()))?;
    if !result.status.success() {
        return Err(format!(
            "linker driver {} failed ({}):\n{}{}",
            linker.to_string_lossy(),
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    fs::rename(&executable, &output)
        .map_err(|e| format!("cannot write executable {}: {e}", output.display()))?;
    Ok(())
}
