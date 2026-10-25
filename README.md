# Pepper

Pepper is an experimental systems programming language with readable syntax,
C interoperability, type inference, functional and object-oriented constructs,
and an emphasis on physical simulation. The original language vision is in
[docs/design.md](docs/design.md).

This repository contains a Rust compiler with an [Inkwell](https://github.com/TheDan64/inkwell)
LLVM backend. It can compile a minimal Hello World program into a native executable.
The parser supports a broader subset than the executable backend.
The original Python/PLYPlus prototype remains in `src/parser.py`, `src/pepper.g`,
and `src/pepper2.g` as a historical reference; Cargo ignores these files.

## Getting started

Install Rust 1.85 or newer, LLVM **19.1** with development libraries, and a system
linker driver (`cc` by default). Inkwell is pinned to 0.11.0 with LLVM 19 dynamic
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

Inspect the generated LLVM IR or the parsed syntax tree:

```sh
cargo run -- emit-ir examples/hello.pepper
cargo run -- check examples/hello.pepper
cargo run -- parse examples/functions.pepper
cargo test --locked --all-targets
```

`check` validates **syntax only**. `parse` prints the syntax tree. `build` and
`emit-ir` also validate the executable subset described below. Source errors include
the filename, line, column, and a caret.

## Executable subset

```text
main() =
    print("Hello world!")
```

The native backend currently accepts exactly one top-level `main()` with no
parameters. Its body can contain calls to the built-in `print` with one String
literal argument, and an optional bare `return`. Each print adds a newline; main
returns exit status zero. Multiple prints, UTF-8 text, percent signs, and escaped
newlines work. Strings containing NUL bytes are rejected because the initial
runtime uses NUL-terminated C strings.

The pipeline is Pepper source -> syntax tree -> semantic validation -> Inkwell
LLVM module -> verified native object -> system linker -> executable. `print`
calls libc's `puts`; there is no generated C and no JIT. The resulting Hello World
executable depends on the system C runtime, not LLVM or the Pepper compiler.

LLVM IR is verified before object emission. The `CC` environment variable can
select the linker driver executable (for example `CC=clang`); it must be an
executable path, not a shell command with arguments. Linking uses temporary files
beside the output; they are cleaned up on failure and success, and failed links
leave an existing output intact. The output parent directory must already exist.

Assignments, user-defined function calls, numeric computation and general type
inference are not executable yet. Unsupported programs fail with diagnostics
rather than silently dropping code. `examples/functions.pepper` remains a parser
example and intentionally cannot be built yet.

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
constraints, units, concurrency, and C interoperability are future work. Unsupported
syntax produces an error. The broader design document is aspirational; this subset
is deliberately small so the compiler can grow in tested steps.

## Compiler layout

| File | Responsibility |
| --- | --- |
| `src/lexer.rs` | Tokens, indentation and delimiter handling |
| `src/ast.rs` | Source-spanned syntax tree |
| `src/parser.rs` | Statements and precedence-based expression parsing |
| `src/diagnostic.rs` | Source locations and diagnostic rendering |
| `src/semantic.rs` | Executable-subset validation and lowered instructions |
| `src/backend.rs` | Inkwell LLVM IR, verification, object emission and linking |
| `src/lib.rs` | Public `pepper::parse` API |
| `src/main.rs` | Command-line interface |
| `tests/` | Parser, semantic, CLI and native compile-and-run tests |

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

GitHub Actions installs LLVM 19, runs these checks, parses both examples, and
compiles and runs Hello World. Tests cover exact output, zero exit status, semantic
errors, linker failures, and temporary-file cleanup.

The implementation sequence is described in [docs/roadmap.md](docs/roadmap.md).
