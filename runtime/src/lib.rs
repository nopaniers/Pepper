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

/// Print a Pepper Int using Rust's numeric formatting.
#[unsafe(no_mangle)]
pub extern "C" fn pepper_runtime_print_int_v1(value: i64) -> i32 {
    std::panic::catch_unwind(|| {
        if print(&value.to_string()).is_ok() {
            0
        } else {
            1
        }
    })
    .unwrap_or(1)
}

/// Print a Pepper Double using Rust's numeric formatting.
#[unsafe(no_mangle)]
pub extern "C" fn pepper_runtime_print_double_v1(value: f64) -> i32 {
    std::panic::catch_unwind(|| {
        if print(&value.to_string()).is_ok() {
            0
        } else {
            1
        }
    })
    .unwrap_or(1)
}

/// Print a Pepper Bool passed as a byte rather than a Rust layout.
#[unsafe(no_mangle)]
pub extern "C" fn pepper_runtime_print_bool_v1(value: u8) -> i32 {
    if value > 1 {
        return 1;
    }
    std::panic::catch_unwind(|| {
        if print(if value == 0 { "false" } else { "true" }).is_ok() {
            0
        } else {
            1
        }
    })
    .unwrap_or(1)
}

/// Report a checked runtime failure. The returned status propagates through
/// Pepper functions to Rust process startup.
#[unsafe(no_mangle)]
pub extern "C" fn pepper_runtime_error_v1(code: i32) -> i32 {
    let message = match code {
        1 => "array index out of bounds",
        2 => "integer division by zero or overflow",
        3 => "negative integer exponent",
        _ => "unknown runtime error",
    };
    let _ = std::panic::catch_unwind(|| {
        let _ = writeln!(io::stderr().lock(), "Pepper runtime error: {message}");
    });
    1
}

/// Integer exponentiation uses wrapping i64 multiplication, like Int arithmetic.
/// The LLVM caller checks the exponent is nonnegative before making this call.
#[unsafe(no_mangle)]
pub extern "C" fn pepper_runtime_pow_int_v1(mut base: i64, exponent: i64) -> i64 {
    let mut exponent = exponent as u64;
    let mut result = 1_i64;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = result.wrapping_mul(base);
        }
        exponent >>= 1;
        base = base.wrapping_mul(base);
    }
    result
}

/// Double exponentiation uses Rust's floating-point implementation.
#[unsafe(no_mangle)]
pub extern "C" fn pepper_runtime_pow_double_v1(base: f64, exponent: f64) -> f64 {
    base.powf(exponent)
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
