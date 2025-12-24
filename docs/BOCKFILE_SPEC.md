# Bockfile Specification v2

A modern, clean, and intuitive format for defining container images.

## Overview

Bockfile v2 is designed to be:
- **Simple** - Clean YAML/TOML/JSON syntax
- **Flexible** - Environment variables, dynamic tags, per-stage configuration
- **Powerful** - Multi-stage builds, security defaults, registry integration

## File Formats

Bockfiles can be written in:
- **YAML** - `Bockfile.yaml` (default, most readable)
- **TOML** - `Bockfile.toml` (Rust-friendly)
- **JSON** - `Bockfile.json` (programmatic)

## Complete Example (YAML)

```yaml
# Base image configuration
base:
  from: alpine:3.19
  alias: builder                    # Optional alias for referencing
  version: env.ALPINE_VERSION       # Override version from env var
  platform: linux/amd64             # Optional platform

# Build arguments with env fallback
args:
  APP_VERSION: "1.0.0"              # Simple value
  DEBUG:
    default: "false"
    env: DEBUG_MODE                 # Read from $DEBUG_MODE
    description: Enable debug mode

# Image metadata
metadata:
  name: my-app
  version: "1.0.0"
  tag: "{{name}}:{{version}}-{{git.sha_short}}"  # Dynamic tag
  description: My awesome application
  authors:
    - "Developer <dev@example.com>"
  license: MIT
  labels:
    org.opencontainers.image.source: https://github.com/org/repo

# Build stages
stages:
  - name: deps
    alias: dependencies
    steps:
      - run: apk add --no-cache build-base curl

  - name: build
    depends: [deps]
    workdir: /app
    env:
      CGO_ENABLED: "0"
    steps:
      - copy:
          copy: [go.mod, go.sum]
          to: /app/
      - run: go mod download
      - copy:
          copy: ["."]
          to: /app/
      - run: go build -o /app/bin/myapp ./cmd/myapp

  - name: runtime
    from: alpine:3.19              # Start fresh for final image
    depends: [build]
    security:                       # Per-stage security
      user: nobody
      capabilities_drop: [ALL]
      readonly_rootfs: true
    steps:
      - copy:
          copy: [/app/bin/myapp]
          to: /usr/local/bin/
          from_stage: build
          chown: "nobody:nobody"
      - user: nobody

# Runtime configuration
runtime:
  entrypoint: ["/usr/local/bin/myapp"]
  cmd: ["serve"]
  workdir: /
  env:
    APP_ENV: production
  ports: ["8080"]
  stop_signal: SIGTERM

# Global security defaults
security:
  no_new_privs: true
  capabilities_drop: [ALL]
  seccomp: default

# Registry integration
registry:
  name: ghcr.io/myorg
  push_on_build: true
  additional_tags:
    - latest
    - "{{git.branch}}"
```

## Sections

### `base`

Defines the base image for the build.

| Field | Type | Description |
|-------|------|-------------|
| `from` | string | Base image reference (e.g., `alpine:3.19`) |
| `alias` | string? | Alias for referencing in multi-stage builds |
| `version` | string? | Version override (supports `env.VAR` syntax) |
| `platform` | string? | Target platform (e.g., `linux/amd64`) |

**Version Resolution:**
- If `version` is set, it overrides the tag in `from`
- `env.VAR_NAME` reads from environment variable
- `${VAR_NAME}` also works

### `args`

Build arguments available throughout the Bockfile.

```yaml
args:
  # Simple value
  APP_NAME: myapp
  
  # With environment fallback
  VERSION:
    default: "1.0.0"
    env: APP_VERSION
    description: Application version
```

**Usage:**
- In steps: `{{args.APP_NAME}}`
- Environment interpolation happens at parse time

### `metadata`

Image metadata and tag configuration.

| Field | Type | Description |
|-------|------|-------------|
| `name` | string? | Image name |
| `version` | string? | Semver version |
| `tag` | string? | Tag template with placeholders |
| `description` | string? | Image description |
| `authors` | string[]? | Author list |
| `license` | string? | License identifier |
| `labels` | map? | OCI labels |

**Tag Placeholders:**
- `{{name}}` - Image name
- `{{version}}` - Version
- `{{git.sha}}` - Full git SHA
- `{{git.sha_short}}` - Short git SHA (7 chars)
- `{{git.branch}}` - Git branch name
- `{{timestamp}}` - Build timestamp

### `stages`

Build stages for multi-stage builds.

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Stage name (required) |
| `alias` | string? | Alternative name |
| `from` | string? | Override base image for this stage |
| `depends` | string[]? | Stages to build first |
| `steps` | Step[]? | Build steps |
| `workdir` | string? | Default working directory |
| `env` | map? | Stage environment variables |
| `security` | SecurityConfig? | Stage-specific security |
| `cache` | CacheConfig? | Caching configuration |

### Steps

#### `run`

Execute a command.

```yaml
# Simple
- run: apk add --no-cache curl

# Detailed
- run:
    run: make build
    workdir: /app
    user: builder
    cache:
      - target: /root/.cache
    network: none
```

#### `copy`

Copy files from context or another stage.

```yaml
# Simple
- copy:
    from: src/
    to: /app/src/

# Detailed
- copy:
    copy: [src/, config/]
    to: /app/
    from_stage: builder        # Copy from another stage
    chown: "1000:1000"
    chmod: "755"
    exclude: ["*.test.go"]
```

#### `add`

Add files with URL/archive support.

```yaml
- add:
    add: https://example.com/file.tar.gz
    to: /app/
    checksum: sha256:abc123...
    extract: true
```

#### Other Steps

```yaml
- workdir: /app
- user: nobody
- env: { KEY: value }
- expose: 8080
- volume: /data
- label: { "key": "value" }
- entrypoint: ["/app/run"]
- cmd: ["--help"]
- healthcheck:
    cmd: ["curl", "-f", "http://localhost/health"]
    interval: 30s
    timeout: 5s
    retries: 3
```

### `runtime`

Container runtime configuration.

| Field | Type | Description |
|-------|------|-------------|
| `entrypoint` | string[]? | Container entrypoint |
| `cmd` | string[]? | Default command |
| `workdir` | string? | Working directory |
| `env` | map? | Environment variables |
| `user` | string? | User to run as |
| `ports` | string[]? | Exposed ports |
| `volumes` | string[]? | Volume mount points |
| `stop_signal` | string? | Stop signal |
| `stop_timeout` | int? | Stop timeout in seconds |

### `security`

Security configuration (global or per-stage).

| Field | Type | Description |
|-------|------|-------------|
| `user` | string? | User to run as |
| `capabilities_add` | string[]? | Capabilities to add |
| `capabilities_drop` | string[]? | Capabilities to drop |
| `no_new_privs` | bool? | Enable no_new_privs |
| `seccomp` | string? | Seccomp profile |
| `apparmor` | string? | AppArmor profile |
| `selinux` | string? | SELinux context |
| `readonly_rootfs` | bool? | Read-only root filesystem |

### `registry`

Registry integration for automatic pushing.

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Registry URL/name |
| `push_on_build` | bool | Push after successful build |
| `additional_tags` | string[]? | Extra tags to push |
| `credentials` | string? | Credential store reference |

## TOML Example

```toml
[base]
from = "alpine:3.19"
alias = "builder"

[args]
APP_VERSION = "1.0.0"

[metadata]
name = "my-app"
version = "1.0.0"
tag = "{{name}}:{{version}}"

[[stages]]
name = "build"
workdir = "/app"

[[stages.steps]]
run = "apk add build-base"

[[stages.steps]]
[stages.steps.copy]
copy = ["src/"]
to = "/app/"

[runtime]
entrypoint = ["/app/run"]
cmd = ["--help"]

[security]
no_new_privs = true
capabilities_drop = ["ALL"]
```

## JSON Example

```json
{
  "base": {
    "from": "alpine:3.19",
    "alias": "builder"
  },
  "metadata": {
    "name": "my-app",
    "version": "1.0.0"
  },
  "stages": [
    {
      "name": "build",
      "steps": [
        { "run": "apk add build-base" }
      ]
    }
  ],
  "runtime": {
    "entrypoint": ["/app/run"]
  }
}
```

## CLI Usage

```bash
# Build with auto-detected format
bock build -f Bockfile.yaml .

# Specify format explicitly
bock build -f Bockfile.toml --format toml .

# Convert between formats
bock convert Bockfile.yaml --to toml > Bockfile.toml
```
