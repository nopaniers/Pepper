# Implementation roadmap

The original design in `design.md` remains the language vision. Rust is the
implementation language; Pepper remains a separate language with its own syntax.

1. **Compiler front end (started).** Source spans, indentation-aware tokens,
   syntax trees, functions, assignments, expressions and useful diagnostics.
   Add conditionals, loops, ranges and modules next, with representative examples.
2. **Semantic analysis (minimal subset implemented).** Validate `main` and
   built-in `print(String literal)`. Next resolve names and scopes, infer variable and return
   types, validate annotated parameters and assignments, and reject invalid type
   changes. Specify value semantics and class mutation before introducing a runtime.
3. **Executable vertical slice (Hello World implemented).** Inkwell 0.11 and
   LLVM 19 emit verified native objects, linked with a Rust runtime by rustc.
   `main()` and printing string literals run end to end. Next add typed values,
   arithmetic and user-defined calls, then a small standard library and a defined
   representation of values and memory management.
4. **Rust crate integration (runtime bridge implemented).** Pepper calls a
   versioned ABI entry point in the `pepper-runtime` Rust crate. Rust owns startup
   and I/O; rustc builds and links the runtime. Next add a Cargo-built bridge for
   external Rust crates, explicit exported signatures, ownership rules and error
   conversions. Direct Rust-native ABI compatibility is not assumed.
5. **Core language expansion.** Classes, structural interfaces, closures,
   collection operations, error handling, resource cleanup and optional brace blocks.
6. **Scientific types and correctness.** Constraints, units, contracts, numerical
   collections and mathematical notation. Define what is checked statically and
   where runtime checks are necessary; arbitrary predicates cannot generally be
   proven by a compiler.
7. **Concurrency.** Specify coroutine scheduling, ownership/sharing, messages,
   cancellation and errors before implementing `co`. GPU execution is a later goal.

Open design decisions include integer sizes and overflow, equality and comparison
chaining, implicit multiplication versus calls/string concatenation, memory and
aliasing, nullability, constraint propagation, and exception behavior. The initial
parser does not settle the runtime or type-system decisions. Its `i64` and `f64`
literal payloads are implementation limits, not a final machine-type specification.
