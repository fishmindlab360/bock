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

### Bockfile Syntax

Bockfiles use YAML for a cleaner, more expressive syntax:

```yaml
base:
  from: "SomeImage" # use version automatically, i.e like if image is like "alpine:latest" and also version defined then priortize version. else fallback to "alpine:latest" else ignore.
  alias: "for example builder" # alias is used to reference base image in stages
  version: env.BASE_IMAGE_VERSION # version is used to reference base image version in stages

args (with env support for fallback):
  arg1 : Value
  arg2: Env.someValue as Default value if not found// Arg Can be used anywhere with root.args.{{arg1}}

metadata:
  name: my-app
  version: "1.0.0"
  tag: my-app:version:git-sha # tag is used to reference image tag in stages as an example
  description: My application

stages:
  - name/alias: build
    steps:
      - run: apk add --no-cache build-base
      - copy:
          - src/
        to: /app/src
      - run: cd /app && make
        workdir: /app
    ... other configs

  - name or alias: runtime
    depends:
      - build
    steps:
      - copy:
          - --from=build
          - /app/bin/myapp
        to: /usr/local/bin/
      - user: nobody
    ... other configs

runtime:
  entrypoint: ["/usr/local/bin/myapp"]
  cmd: ["--help"]
  env:
    APP_ENV: production
  workdir: /app

security (with per stage level security configuration):
  user: nobody
  capabilities:
    drop:
      - ALL

registry:
    name: some_registry
    push_on_build: true // Push on sucess build.
```

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

Environment variables also work:
```bash
export BOCK_REGISTRY_DOCKER_IO_USERNAME=user
export BOCK_REGISTRY_DOCKER_IO_PASSWORD=token
```

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
