# 🐳 Bock Container Ecosystem

A modern, spec-driven container ecosystem built entirely in Rust for performance, safety, and developer experience.

## Components

| Crate | Description | Status |
|-------|-------------|--------|
| **bock** | Low-level container runtime | 🚧 In Development |
| **bock-runtime** | Spec-driven image builder | 📋 Planned |
| **bockrose** | Multi-container orchestration | 📋 Planned |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      bockrose                                │
│              (Multi-Container Orchestration)                │
├─────────────────────────────────────────────────────────────┤
│                    Bock Runtime                             │
│                  (Image Builder)                            │
├─────────────────────────────────────────────────────────────┤
│                        Bock                                 │
│                 (Container Runtime)                         │
├──────────┬──────────┬──────────┬──────────┬────────────────┤
│Namespaces│ Cgroups  │Filesystem│ Network  │   Security     │
└──────────┴──────────┴──────────┴──────────┴────────────────┘
```

## Quick Start

### Prerequisites

- Rust 1.85+ (2024 edition)
- Linux kernel 5.10+ (for cgroups v2)
- Root privileges (for container operations)

### Building

```bash
# Build all crates
cargo build --workspace

# Build release
cargo build --workspace --release

# Run tests
cargo test --workspace
```

### Usage

```bash
# Run a container
bock run --rm alpine:latest echo "Hello, Bock!"

# Build an image
bock-runtime build -t myapp:latest .

# Run a multi-container stack
bockrose up -d
```

## Documentation

- [Implementation Plan](docs/implementation-plan.md)
- [Bockfile Specification](docs/bockfile-spec.md)
- [bockrose Specification](docs/bockrose-spec.md)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
