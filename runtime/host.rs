//! Rust-owned process startup for an LLVM-compiled Pepper program.
unsafe extern "C" {
    fn pepper_main() -> i32;
}

fn main() -> std::process::ExitCode {
    // SAFETY: Pepper emits this parameterless entry point with the declared ABI.
    pepper_runtime::exit_code(unsafe { pepper_main() })
}
