//! Rust runtime for LLVM-compiled Pepper programs.
//!
//! Rust implementations stay behind an explicit, versioned ABI bridge. No Rust
//! String, reference, trait object or other compiler-dependent layout crosses it.
use std::{
    io::{self, Write},
    process::ExitCode,
};

fn write_line(writer: &mut impl Write, text: &str) -> io::Result<()> {
    writer.write_all(text.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Print a UTF-8 string through Rust's standard I/O library, adding a newline.
pub fn print(text: &str) -> io::Result<()> {
    write_line(&mut io::stdout().lock(), text)
}

/// Convert the Pepper runtime's success/failure status to a Rust process result.
pub fn exit_code(status: i32) -> ExitCode {
    if status == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Version 1 bridge called from Pepper-generated LLVM code.
///
/// Returns 0 on success, 1 for invalid UTF-8, invalid length or an output error.
/// Null is allowed only for empty strings. Strings are borrowed for this call;
/// they are never stored or freed by the runtime. Embedded NUL bytes are supported.
/// The C calling convention is solely a stable binary boundary for Rust code.
///
/// # Safety
/// If non-null and `0 < len <= isize::MAX`, `data` must point to `len` readable
/// bytes in a single allocation, and those bytes must not change during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pepper_runtime_print_v1(data: *const u8, len: usize) -> i32 {
    if len > isize::MAX as usize || (data.is_null() && len != 0) {
        return 1;
    }
    // Contain Rust panics at the bridge rather than unwinding into LLVM code.
    std::panic::catch_unwind(|| {
        let bytes = if len == 0 {
            &[]
        } else {
            // SAFETY: the caller guarantees a valid, immutable allocation for len bytes.
            unsafe { std::slice::from_raw_parts(data, len) }
        };
        let Ok(text) = std::str::from_utf8(bytes) else {
            return 1;
        };
        if print(text).is_ok() { 0 } else { 1 }
    })
    .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_printing_preserves_utf8_nul_and_newlines() {
        let mut bytes = Vec::new();
        write_line(&mut bytes, "🌶\0Pepper\n").unwrap();
        assert_eq!(bytes, "🌶\0Pepper\n\n".as_bytes());
    }

    #[test]
    fn output_errors_are_returned() {
        struct BrokenOutput;
        impl Write for BrokenOutput {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(
            write_line(&mut BrokenOutput, "hello").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
    }

    #[test]
    fn bridge_rejects_invalid_inputs_without_dereferencing_them() {
        // SAFETY: null pointers with nonzero lengths are explicitly rejected.
        assert_eq!(unsafe { pepper_runtime_print_v1(std::ptr::null(), 1) }, 1);
        // SAFETY: invalid UTF-8 is still a valid immutable allocation to read.
        assert_eq!(unsafe { pepper_runtime_print_v1([0xff].as_ptr(), 1) }, 1);
    }
}
