# Contributing to Bock

Thank you for your interest in contributing to Bock! This guide will help you get started.

## Development Setup

### Prerequisites

- Rust 1.85+ (we use edition 2024)
- Linux kernel 5.0+ (for cgroups v2)
- `libseccomp-dev` (for seccomp support)

### Building

```bash
# Clone the repository
git clone https://github.com/bock-containers/bock.git
cd bock

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run with verbose logging
RUST_LOG=debug cargo run -p bock -- <command>
```

### Project Structure

```
bock/
├── crates/
│   ├── bock/           # Core runtime
│   ├── bock-runtime/   # Image builder
│   ├── bockrose/       # Orchestrator
│   ├── bock-image/     # Image management
│   ├── bock-network/   # Networking
│   ├── bock-oci/       # OCI types
│   └── bock-common/    # Shared utilities
├── docs/               # Documentation
└── tests/              # Integration tests
```

## Coding Standards

### Rust Style

- Follow `rustfmt` defaults
- Use `clippy` with pedantic lints
- Prefer `rustix` over raw `libc` when possible
- Document all public APIs

### Error Handling

- Use `BockError` from `bock-common`
- Provide context in error messages
- Don't panic in library code

### Unsafe Code

- Minimize unsafe blocks
- Document safety invariants with `// SAFETY:` comments
- Add module-level `#![allow(unsafe_code)]` with justification

Example:
```rust
// SAFETY: fd is a valid open file descriptor obtained from open().
// The buffer is properly sized and the read length is checked.
unsafe { libc::read(fd, buf.as_mut_ptr() as *mut c_void, buf.len()) }
```

### Testing

- Unit tests in same file (`#[cfg(test)]`)
- Integration tests in `tests/`
- Property-based tests for parsers
- Use `tempfile` for filesystem tests

## Pull Request Process

1. **Fork** the repository
2. **Create a branch** (`git checkout -b feature/amazing-feature`)
3. **Make changes** with tests
4. **Run checks**:
   ```bash
   cargo fmt --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```
5. **Commit** with clear message
6. **Push** and create PR

### Commit Messages

Follow conventional commits:
- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation
- `refactor:` Code restructuring
- `test:` Adding tests
- `chore:` Maintenance

## Areas for Contribution

### Good First Issues

- Add missing documentation
- Improve error messages
- Add unit tests
- Fix clippy warnings

### Intermediate

- Implement missing OCI features
- Add network policy features
- Improve caching strategies

### Advanced

- Performance optimizations
- New storage drivers
- Kubernetes integration

## Getting Help

- Open an issue for bugs
- Discussions for questions
- Read the architecture docs in `docs/`

## License

By contributing, you agree that your contributions will be licensed under Apache-2.0 OR MIT.
