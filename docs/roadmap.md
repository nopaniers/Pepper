# Implementation roadmap

The original design in `design.md` remains the language vision. Rust is the
implementation language; Pepper remains a separate language with its own syntax.

1. **Compiler front end (started).** Source spans, indentation-aware tokens,
   syntax trees, functions, assignments, expressions and useful diagnostics.
   Add conditionals, loops, ranges and modules next, with representative examples.
2. **Semantic analysis (functions and values implemented).** Resolve functions,
   parameters and local variables; infer scalar return/local types, validate
   assignments and calls, and check homogeneous fixed-size local arrays. Next
   support recursive return inference, richer scopes and control-flow type checking.
3. **Executable vertical slice (both examples implemented).** Inkwell 0.11 and
   LLVM 19 emit verified native functions, scalar values, calls, numeric operations
   and checked array indexing. The Rust runtime formats strings and numbers, supplies
   powers, and links with Pepper objects through rustc. Next add control flow and
   define dynamic collection storage, ownership and a small standard library.
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

The initial numeric implementation uses wrapping signed 64-bit Int arithmetic and
64-bit Double values. These are provisional. Open design decisions include the
full machine-type family, overflow policies, equality and comparison
chaining, implicit multiplication versus calls/string concatenation, memory and
aliasing, nullability, constraint propagation, and exception behavior. The initial
parser does not settle the runtime or type-system decisions. The current numeric rules are not a final machine-type specification.
