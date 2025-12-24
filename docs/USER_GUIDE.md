# Bock User Guide

## Getting Started

### Installation

```bash
# From source
cargo install --path crates/bock

# Or build all tools
cargo build --release --workspace
```

### Quick Start

```bash
# Pull an image
bock pull alpine:latest

# Run a container
bock run alpine echo "Hello, Bock!"

# Interactive shell
bock run -it alpine /bin/sh

# List containers
bock ps

# Stop container
bock stop <container-id>
```

## Building Images

### Bockfile v2 Syntax

Bockfiles use a clean, modern syntax with YAML, TOML, or JSON support.

#### Simple Example (YAML)

```yaml
base:
  from: alpine:3.19

metadata:
  name: my-app
  version: "1.0.0"

stages:
  - name: build
    steps:
      - run: apk add --no-cache curl
      - copy:
          copy: [src/]
          to: /app/

runtime:
  cmd: ["/app/run"]
```

#### Full-Featured Example

```yaml
# Base image with env version override
base:
  from: alpine
  alias: builder
  version: env.ALPINE_VERSION    # Use $ALPINE_VERSION if set

# Build args with environment fallback
args:
  APP_VERSION:
    default: "1.0.0"
    env: VERSION                 # Use $VERSION if set
  DEBUG_MODE: "false"

# Image metadata with dynamic tag
metadata:
  name: my-app
  version: "{{args.APP_VERSION}}"
  tag: "{{name}}:{{version}}-{{git.sha_short}}"
  description: My application

# Multi-stage build
stages:
  - name: deps
    steps:
      - run: apk add --no-cache build-base

  - name: build
    depends: [deps]
    workdir: /app
    steps:
      - copy:
          copy: ["."]
          to: /app/
      - run: make build

  - name: runtime
    from: alpine:3.19           # Fresh base for final image
    depends: [build]
    security:                    # Per-stage security
      user: nobody
      capabilities_drop: [ALL]
    steps:
      - copy:
          copy: [/app/bin/myapp]
          to: /usr/local/bin/
          from_stage: build
      - user: nobody

# Runtime configuration
runtime:
  entrypoint: ["/usr/local/bin/myapp"]
  cmd: ["--help"]
  env:
    APP_ENV: production
  workdir: /app

# Global security defaults
security:
  user: nobody
  no_new_privs: true
  capabilities_drop: [ALL]

# Auto-push to registry
registry:
  name: ghcr.io/myorg
  push_on_build: true
```

### Key Features

| Feature | Description |
|---------|-------------|
| `base.version: env.VAR` | Override image tag from environment |
| `args.X.env: VAR` | Fallback to env var for arg value |
| `{{git.sha_short}}` | Dynamic tag with git SHA |
| `stages[].security` | Per-stage security config |
| `registry.push_on_build` | Auto-push after successful build |

### Building

```bash
# Build with default tag
bock build -f Bockfile.yaml .

# Build with custom tag
bock build -t myapp:v1.0 .

# Build with args
bock build --build-arg APP_VERSION=2.0 .

# No cache
bock build --no-cache .

# Target specific stage
bock build --target build .
```

## Container Management

### Running Containers

```bash
# Basic run
bock run <image> <command>

# Interactive with TTY
bock run -it <image> /bin/sh

# Detached (background)
bock run -d <image> <command>

# With port mapping
bock run -p 8080:80 nginx

# With volume mount
bock run -v /host/path:/container/path <image>

# With environment variables
bock run -e DATABASE_URL=postgres://... <image>
```

### Lifecycle Commands

```bash
# List running containers
bock ps

# List all containers
bock ps -a

# Stop container
bock stop <container-id>

# Remove container
bock rm <container-id>

# View logs
bock logs <container-id>

# Execute in running container
bock exec -it <container-id> /bin/sh
```

## Image Management

```bash
# List images
bock images

# Pull from registry
bock pull <image>:<tag>

# Push to registry
bock push <image>:<tag>

# Remove image
bock rmi <image>

# Inspect image
bock inspect <image>
```

## Registry Authentication

### Login

```bash
# Interactive login
bock login docker.io

# With credentials
bock login -u username -p password registry.example.com
```

### Credential Storage

Credentials are stored at `~/.bock/credentials.json` in Docker-compatible format.

Supports multiple backends:
- **File** (default) - `~/.bock/credentials.json`
- **Keyring** - Native OS keychain
- **Pass** - password-store
- **Environment** - `BOCK_REGISTRY_<HOST>_USERNAME/PASSWORD`

## Networking

### Network Modes

```bash
# Bridge (default)
bock run --network bridge <image>

# Host network
bock run --network host <image>

# No network
bock run --network none <image>
```

### Port Publishing

```bash
# Map host port to container port
bock run -p 8080:80 nginx

# Random host port
bock run -p 80 nginx

# Multiple ports
bock run -p 8080:80 -p 8443:443 nginx
```

## Volumes

```bash
# Bind mount
bock run -v /host/path:/container/path <image>

# Read-only
bock run -v /host/path:/container/path:ro <image>

# Named volume
bock run -v mydata:/data <image>
```

## Resource Limits

```bash
# Memory limit
bock run --memory 512m <image>

# CPU limit
bock run --cpus 2 <image>

# Combine limits
bock run --memory 1g --cpus 0.5 <image>
```

## Security

### Running as Non-root

```bash
bock run --user nobody <image>
```

### Capabilities

```bash
# Drop all capabilities
bock run --cap-drop ALL <image>

# Add specific capability
bock run --cap-add NET_ADMIN <image>
```

### Read-only Filesystem

```bash
bock run --read-only <image>
```

## Troubleshooting

### Debug Mode

```bash
RUST_LOG=debug bock run alpine
```

### Common Issues

**Permission denied**: Run with sudo or configure rootless mode.

**Image not found**: Check registry credentials and image name.

**Port already in use**: Choose a different host port.

## See Also

- [Bockfile Specification](./BOCKFILE_SPEC.md) - Complete format reference
- [API Reference](./API_REFERENCE.md) - Rust API documentation
- [Architecture](./ARCHITECTURE.md) - System design
