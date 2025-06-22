# Avoid Async Rust for Core Engine Simplicity

- Status: accepted
- Deciders: Development Team
- Date: 2024-12-19

Technical Story: As we adopt Rust, we need to decide on a concurrency model that fits the project's goals of simplicity and performance for a CPU-bound workload.

## Context and Problem Statement

The core of the lighting engine involves intensive mathematical calculations (ray casting, collision checks, etc.), which are fundamentally CPU-bound tasks. Rust offers powerful features for concurrency, most notably its `async/await` syntax for I/O-bound operations and libraries like `rayon` for data parallelism.

Using `async` introduces significant complexity, including `async` runtimes, the `Future` trait, and the "function coloring" problem, which can complicate the codebase unnecessarily. The question is how to best structure the code for performance without sacrificing the project's core value of simplicity.

## Decision Drivers

- **Code Simplicity**: The chosen approach must keep the core logic as simple and maintainable as possible.
- **Performance**: The solution must be performant for CPU-bound calculations.
- **Low Complexity**: Avoid introducing complex abstractions or runtimes that are not essential for the task.
- **Easy Integration**: The compiled code should be easy to integrate into various targets like WebAssembly (via Web Workers) and native game engines.

## Considered Options

- **Use `async` Rust**: Employ an `async` runtime like `tokio` or `async-std` to manage tasks.
- **Use standard synchronous Rust**: Write the core logic as simple, blocking functions.
- **Use synchronous Rust with `rayon` for parallelism**: Write synchronous code but use a data-parallelism library for performance scaling.

## Decision Outcome

Chosen option: **"Use standard synchronous Rust, with `rayon` for parallelism"**.

This approach is the best fit for the project. The core engine logic will be written as simple, synchronous functions. This keeps the code clean, easy to understand, and debug. `async/await` is the wrong tool for this CPU-bound problem and would add needless complexity.

For future performance scaling, especially to support multiple light sources efficiently, we will use a library like `rayon`. It allows for easy and near-transparent conversion of sequential iterators into parallel ones, effectively utilizing multiple CPU cores without the cognitive overhead of `async` or manual thread management.

### Positive Consequences

- **Maximum Simplicity**: The codebase remains clean, synchronous, and easy to reason about.
- **No `async` Overhead**: We avoid the complexity of `async` runtimes, `Future`s, `Pin`ning, and function coloring.
- **Targeted Performance**: `rayon` is specifically designed for CPU-bound parallelism, offering a direct and efficient path to performance optimization.
- **Easy Integration**: Synchronous Rust/WASM code is trivial to run in a Web Worker, preventing UI blocking. Native integrations are also more straightforward.

### Negative Consequences

- **Not Suited for I/O**: If the project's scope were to pivot to heavy I/O operations (e.g., streaming assets over a network), this decision would need to be re-evaluated. This is considered highly unlikely.

## Pros and Cons of the Options

### Use `async` Rust

- Good, because it is excellent for managing a high number of I/O-bound tasks concurrently.
- Bad, because it provides no benefit for CPU-bound computations.
- Bad, because it introduces significant complexity through runtimes and `async`-specific concepts.
- Bad, because it "colors" the codebase, forcing `async` through the call stack.

### Use standard synchronous Rust

- Good, because it is the simplest and most direct way to write the code.
- Good, because it is perfectly suited for CPU-bound logic.
- Bad, because it does not provide a strategy for parallelism on its own, which could be a limitation for scaling.

### Use synchronous Rust with `rayon` for parallelism

- Good, because it combines the simplicity of synchronous code with a clear, powerful strategy for performance scaling.
- Good, because `rayon` is extremely easy to adopt and often only requires changing a single line of code to parallelize an iterator.
- Good, because it is the ideal solution for CPU-bound, data-parallel problems like this one.
- No significant cons for this project's use case.

## Links

- [The `rayon` Crate](https://crates.io/crates/rayon)
- [Rust Blog: `async/await` is for I/O](https://blog.rust-lang.org/2019/11/22/Rust-2020-and-beyond.html)
- [Understanding `async` vs. Threading in Rust](https://www.fpcomplete.com/blog/async-vs-threading-rust/) 