# Pepper

Pepper is an experimental systems programming language with readable syntax,
Rust crate interoperability, type inference, functional and object-oriented constructs,
and an emphasis on physical simulation. The original language vision is in
[docs/design.md](docs/design.md).

This repository contains a Rust compiler with an [Inkwell](https://github.com/TheDan64/inkwell)
LLVM backend. It compiles Hello World and typed function/array examples into native executables.
The parser supports a broader subset than the executable backend.
The original Python/PLYPlus prototype remains in `src/parser.py`, `src/pepper.g`,
and `src/pepper2.g` as a historical reference; Cargo ignores these files.

## Getting started

Install Rust 1.85 or newer (including `rustc`), LLVM **19.1** with development
libraries, and the platform linker tools required by Rust. Inkwell is pinned to 0.11.0 with LLVM 19 dynamic
linking. The Cargo lockfile pins the resolved Rust dependencies. Native targets
currently enabled are x86 and AArch64. The compiler has been tested locally on
Intel macOS; CI is configured for Ubuntu 24.04.

On macOS with Homebrew:

```sh
brew install llvm@19
export LLVM_SYS_191_PREFIX="$(brew --prefix llvm@19)"
```

On Ubuntu 24.04:

```sh
sudo apt-get update
sudo apt-get install llvm-19-dev clang
export LLVM_SYS_191_PREFIX=/usr/lib/llvm-19
```

The full LLVM installation is needed; Apple's Command Line Tools alone do not
provide the required development libraries. Alternatively, put a machine-specific
LLVM path in `.cargo/config.toml` (gitignored):

```toml
[env]
LLVM_SYS_191_PREFIX = { value = "/path/to/llvm-19", force = false }
```

From the repository, compile and run Hello World:

```sh
cargo build --locked
cargo run -- build examples/hello.pepper -o target/hello
./target/hello
# Hello from Pepper!
```

Compile and run the function example:

```sh
cargo run -- build examples/functions.pepper -o target/functions
./target/functions
# 3
```

Inspect the generated LLVM IR or the parsed syntax tree:

```sh
cargo run -- emit-ir examples/hello.pepper
cargo run -- check examples/hello.pepper
cargo run -- parse examples/functions.pepper
cargo test --locked --workspace --all-targets
```

`check` validates **syntax only**. `parse` prints the syntax tree. `build` and
`emit-ir` also validate the executable subset described below. Source errors include
the filename, line, column, and a caret.

## Executable subset

```text
main() =
    print("Hello world!")
```

The native backend accepts top-level functions with explicitly typed parameters,
a parameterless `main()`, and inferred local and return types. A function's last
expression is its result; explicit `return value` is also supported. A body ending
with an assignment or bare `return` returns Unit. `main` discards expression
results and accepts only bare returns. Forward calls work; duplicate definitions,
unknown variables, invalid argument counts and type mismatches are diagnosed.
Every function is checked, including unused ones.

The executable value types are `Int`, `Double`, `Bool`, `String`, and local,
one-dimensional arrays of scalar values. Assignments may have type annotations;
reassignment preserves the variable's type. Integers can widen to Double in
arithmetic, calls or assignments, but Double does not narrow implicitly to Int.
Functions can accept and return scalar values; arrays cannot cross function
boundaries yet. Empty arrays cannot yet infer an element type. Arrays are fixed
length, copied by value, and indexed by Int with runtime bounds checks; negative
indexes are errors. Element assignment and slices are not implemented yet.

Supported numeric operations are `+ - * / % ^`, unary signs, and comparisons.
`not` accepts Bool; short-circuit `and`/`or` remain parser-only. As an initial
implementation rule, Int is signed 64-bit with wrapping addition, subtraction,
multiplication, negation and nonnegative integer powers. Integer division truncates
toward zero. Division by zero, signed division overflow and negative integer
exponents report runtime errors and exit with status one. Double is a 64-bit float
with IEEE arithmetic and Rust floating-point powers. These numerical rules are
provisional rather than a final systems-language specification.

`print` accepts one Int, Double, Bool or String, formats numbers through Rust, and
adds a newline. Multiple prints, UTF-8 text, percent signs, escaped newlines and
embedded NUL bytes work. Strings cross the runtime bridge as borrowed UTF-8 bytes
with an explicit byte length. Output failures propagate through calls and exit
with status one. Successful execution returns status zero.

The pipeline is Pepper source -> syntax tree -> semantic validation -> Inkwell
LLVM module -> verified native object -> Rust runtime crate and startup -> executable.
`print` calls the `pepper-runtime` Rust crate, which uses Rust standard I/O. The
compiler embeds the runtime sources and uses `rustc` to compile an `rlib` and link
it with the native Pepper object. There is no generated C and no JIT. The result
runs without LLVM or the Pepper compiler; Rust still uses platform system libraries.

LLVM IR is verified before object emission. `RUSTC` selects the Rust compiler
executable (default: `rustc`); it must be an executable path, not a shell command
with arguments. The selected compiler builds both the runtime and Rust startup
so their Rust ABI matches. `CC` no longer selects Pepper's build driver; rustc
chooses its platform linker. Builds use temporary files beside the output; they
are cleaned up on failure and success, and failed builds leave an existing output
intact. The output parent directory must already exist.

Nested functions, recursive return-type inference, control-flow statements and
general Rust crate imports are not executable yet. Unsupported programs fail with
diagnostics rather than silently dropping code. Both repository examples build
and run; the parser still accepts a larger language than the backend.

## Parser syntax

```text
add(x: Int, y: Int) = x + y

main() =
    numbers: Int[] = [1, 2, 3]
    result = add(numbers[0], numbers[1])
    print(result)
```

Supported:

- Inline and indented function bodies, nested functions, explicitly typed parameters.
- Assignments with optional named type annotations, array types and nullable types.
- Integers, floats, double-quoted strings, booleans (`true` / `false`), and arrays.
- Calls, member access, indexing, grouping, and explicit `return`.
- Arithmetic (`+ - * / % ^`), comparisons, and `and`, `or`, `not`.
- `#` comments, blank lines, and multiline expressions inside `()` and `[]`.

Indentation uses spaces and must match an outer level when dedenting. Exponentiation
is right-associative and binds more tightly than unary minus. Multiplication is
currently explicit. Identifiers currently use ASCII letters, digits, and underscores.

Classes, control flow, ranges, imports, brace-delimited blocks, implicit multiplication,
constraints, units, concurrency, and general Rust crate imports are future work. Unsupported
syntax produces an error. The broader design document is aspirational; this subset
is deliberately small so the compiler can grow in tested steps.

## Rust crate interoperability

The primary interoperability goal is **calling Rust crates from Pepper**. The
current runtime is the first working example: LLVM-generated Pepper code calls
Rust's `pepper-runtime` crate, and Rust owns process startup and printing.

The bridge uses `extern "C"` as a stable calling convention for Rust functions.
This does not require a C implementation. Rust's native ABI and layouts are not
stable, so Pepper must not guess how to pass a Rust `String`, `Vec`, reference or
trait object. The versioned print bridge takes `(*const u8, usize)` and returns an
`i32` status; it borrows the bytes only during the call and does not unwind into
Pepper. `runtime/src/lib.rs` documents its safety contract.

Importing arbitrary Cargo dependencies is not implemented yet. The next stage
is a Cargo-built Rust bridge with explicit exported functions and Pepper type
signatures. Rust wrapper functions can then call crates through normal Rust APIs;
Pepper will call the wrappers. Ownership, error conversion and panic handling
must be defined at each boundary. See [docs/rust-interop.md](docs/rust-interop.md).

## Compiler layout

| File | Responsibility |
| --- | --- |
| `src/lexer.rs` | Tokens, indentation and delimiter handling |
| `src/ast.rs` | Source-spanned syntax tree |
| `src/parser.rs` | Statements and precedence-based expression parsing |
| `src/diagnostic.rs` | Source locations and diagnostic rendering |
| `src/semantic.rs` | Name resolution, local/return type inference and typed IR |
| `src/codegen.rs` | Typed LLVM functions, values, calls and runtime guards |
| `src/backend.rs` | Inkwell LLVM IR, object emission and rustc-driven linking |
| `runtime/src/lib.rs` | Rust runtime crate, safe I/O API and versioned ABI bridge |
| `runtime/host.rs` | Rust process startup calling the Pepper entry point |
| `src/lib.rs` | Public `pepper::parse` API |
| `src/main.rs` | Command-line interface |
| `tests/` | Parser, semantic, CLI and native compile-and-run tests |

## Development

```sh
cargo fmt --all -- --check
rustfmt --edition 2024 --check runtime/host.rs
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
```

GitHub Actions installs LLVM 19, runs these checks, parses both examples, and
compiles and runs both examples. Tests cover function calls, type inference, numeric
operations, array copies/indexing, runtime errors, Rust compiler failures, output
failures, and temporary-file cleanup.

The implementation sequence is described in [docs/roadmap.md](docs/roadmap.md).
