# Use Rust for Core Engine Instead of C3

- Status: accepted

Technical Story: The project needs a robust, modern, and maintainable language for its core logic to ensure long-term viability and developer productivity.

## Context and Problem Statement

The project was initially implemented in C3, a promising but relatively obscure systems language. While C3 offers C-like syntax and some modern features, its small ecosystem presents challenges for collaboration, tooling, and long-term maintenance. The choice of programming language is critical for the project's success, impacting everything from hiring and onboarding to the quality of available libraries and support.

How can we ensure the project is built on a language that is powerful, safe, and well-supported by the community and modern development tools?

## Decision Drivers

- **Developer Ecosystem**: The language should have a large, active community.
- **Tooling Maturity**: Robust build tools, package managers, and editor support are essential.
- **Collaborator Accessibility**: The language should be familiar to a wider pool of developers to encourage contributions.
- **AI/LLM Support**: A language well-understood by AI assistants improves development velocity.
- **Performance & Safety**: The language must be suitable for high-performance, low-level systems programming without sacrificing safety.
- **Long-Term Viability**: The language should have a stable trajectory and strong backing.

## Considered Options

- **Stick with C3**
- **Migrate to Zig**
- **Migrate to Rust**

## Decision Outcome

Chosen option: "Migrate to Rust", because it offers the best combination of performance, memory safety, a mature ecosystem, and widespread adoption. This choice makes the project more accessible for collaboration, easier to maintain, and better supported by modern tools, including AI code assistants. The vibes will be immaculate.

### Positive Consequences

- **Improved Collaboration**: Rust's popularity makes it easier to find and onboard new contributors.
- **Mature Tooling**: Access to Cargo, rust-analyzer, and a vast ecosystem of libraries (crates).
- **Enhanced Safety**: Rust's ownership model and borrow checker prevent entire classes of bugs at compile time.
- **Better AI Support**: LLMs have extensive knowledge of Rust, accelerating development and debugging.
- **Strong Community**: A large and welcoming community provides excellent resources and support.
- **Proven Track Record**: Rust is used successfully in production across many industries.

### Negative Consequences

- **Steeper Learning Curve**: Rust's concepts (ownership, borrowing, lifetimes) can be challenging for developers new to the language.
- **Initial Migration Effort**: Converting the existing C3 codebase to Rust will require time and effort.
- **Verbosity**: Rust can sometimes be more verbose than C3 or Zig for certain tasks.

## Pros and Cons of the Options

### Stick with C3

- Good, because it is already implemented.
- Good, because it is syntactically simple and close to C.
- Bad, because it has a very small community and ecosystem.
- Bad, because it's an obscure language, making it hard to find collaborators.
- Bad, because poor support from tooling and AI assistants.

### Migrate to Zig

- Good, because it offers a simpler, C-like syntax that is closer to the original C3 code.
- Good, because it has excellent C interoperability and a focus on simplicity.
- Good, because it is gaining popularity and has a passionate community.
- Bad, because its ecosystem and tooling are less mature than Rust's.
- Bad, because it is still relatively obscure compared to Rust, which could limit collaboration.

### Migrate to Rust

- Good, because it has a massive, mature ecosystem and excellent tooling (Cargo).
- Good, because its safety guarantees prevent common systems programming bugs.
- Good, because it has widespread adoption and is highly sought after by developers.
- Good, because it has excellent support from LLMs and development tools.
- Bad, because the learning curve can be steep.
- Bad, because the initial migration requires a significant time investment.

## Links

- [The Rust Programming Language](https://www.rust-lang.org/)
- [Zig Programming Language](https://ziglang.org/)
- [C3 Programming Language](https://c3-lang.org/) 