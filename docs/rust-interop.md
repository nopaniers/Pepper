# Calling Rust crates from Pepper

Pepper's primary interoperability direction is calling Rust crates. LLVM remains
the Pepper backend; Rust implements adapters and runtime services.

## Working implementation

1. Pepper's LLVM object defines `pepper_main() -> i32`.
2. String printing calls `pepper_runtime_print_v1(data, byte_len) -> i32`; numeric
   and Bool printing use typed runtime bridges. Power operations also use Rust
   runtime functions.
3. The Rust `pepper-runtime` crate exports that bridge function and provides safe
   printing via Rust's standard I/O. It returns 0 on success and 1 on invalid input
   or output failure. Pepper propagates a failing status without executing later
   statements. Strings can contain embedded NUL bytes.
4. `rustc` compiles the bundled crate as an `rlib`, then compiles Rust process
   startup and links the Pepper object and crate. The runtime is bundled with the
   compiler, so an installed compiler does not need the repository at build time.

Pepper-to-Pepper function calls use an internal status-returning convention, with
scalar results written through an output pointer only on success. Runtime errors
therefore propagate through calls to `pepper_main` without loading an unwritten
result. This internal convention is not an external Rust crate import interface.

The runtime crate is a Cargo workspace member, independently testable with
`cargo test -p pepper-runtime`. Both runtime and startup use the selected `RUSTC`
executable in each build, so their internal Rust interfaces are compiler-matched.
The first implementation targets the host machine and recompiles the small runtime
per build. Runtime caching and cross compilation are future work.

## Binary boundary

The [Rust reference](https://doc.rust-lang.org/reference/items/external-blocks.html)
does not guarantee a stable Rust ABI. A Rust crate's native function symbol and
argument layout cannot be inferred from a Pepper signature. The LLVM/Rust bridge
therefore uses `extern "C"` with explicit primitive representations. This is a
calling convention, not a C runtime implementation.

The current bridge borrows a pointer and a native-width byte length only for the
call. Pepper retains ownership. The Rust side validates UTF-8, never retains the
pointer, never frees it, and contains unwinding Rust panics. LLVM uses the target's
pointer-sized integer for `usize`. Zero means success; errors become exit status
one. Rust I/O may still depend on operating-system libraries internally.

Rust `String`, `Vec`, references, enums, closures and trait objects must not be
passed directly across the bridge. Future adapters should use borrowed buffers,
fixed-width scalar values or opaque handles with explicit release functions.

## Next stage: external Cargo crates

General crate import syntax and dependency declarations are not implemented yet.
The intended sequence is:

1. Add a Cargo-built bridge crate with ordinary Cargo dependencies and a lockfile.
2. Describe exported bridge functions and supported Pepper types in an explicit
   interface manifest; validate calls before code generation.
3. Implement Rust wrapper functions that call the dependency's normal Rust API.
   Cargo/rustc handles generics, traits and dependency linking inside Rust.
4. Export versioned symbols with explicit argument and result representations.
   Specify ownership, borrowed lifetimes, errors and panic behavior for each type.
5. Extend Pepper's resolver and LLVM lowering to invoke those registered symbols.

A first external-crate example should accept and return simple scalar values or
borrowed strings, with an integration test compiling and executing a Pepper caller.
Arbitrary Rust type import, automatic generic instantiation, and shared ownership
across languages need later design work.
