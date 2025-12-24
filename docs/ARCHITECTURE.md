# Bock Architecture Documentation

## Overview

Bock is a modern container ecosystem built in Rust, consisting of:

- **bock** - Core container runtime
- **bock-runtime** - Image builder (Bockfile)
- **bockrose** - Multi-container orchestrator
- **bock-image** - Image management and registry
- **bock-network** - Container networking
- **bock-oci** - OCI specification types
- **bock-common** - Shared utilities

## Crate Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        bockrose                              │
│              (Multi-container Orchestration)                 │
├─────────────────────────────────────────────────────────────┤
│         bock-runtime              │      bock-image          │
│        (Image Builder)            │   (Registry/Storage)     │
├───────────────────────────────────┴──────────────────────────┤
│                          bock                                 │
│                   (Container Runtime)                         │
├─────────────────────────────────────────────────────────────┤
│       bock-network          │        bock-oci                │
│     (Networking Stack)      │    (OCI Spec Types)            │
├─────────────────────────────┴────────────────────────────────┤
│                       bock-common                             │
│                   (Shared Utilities)                          │
└─────────────────────────────────────────────────────────────┘
```

## Core Runtime (`bock`)

### Modules

```
bock/
├── cgroup/          # Cgroup v1/v2 resource management
│   ├── manager.rs   # CgroupManager - CPU, memory, I/O limits
│   └── v1.rs        # Cgroup v1 fallback support
├── exec/            # Process execution
│   ├── console.rs   # PTY attachment via Unix sockets
│   ├── init.rs      # Container init process (PID 1)
│   ├── process.rs   # Process spawning
│   ├── pty.rs       # Pseudo-terminal handling
│   └── stdio.rs     # Stdin/stdout/stderr management
├── filesystem/      # Filesystem operations
│   ├── mount.rs     # Mount operations
│   ├── pivot.rs     # pivot_root for rootfs
│   ├── layers.rs    # Copy-on-write layer management
│   └── volumes.rs   # Volume mounts
├── namespace/       # Linux namespace management
│   ├── manager.rs   # Namespace creation/unshare
│   └── userns.rs    # UID/GID mapping
├── runtime/         # Container lifecycle
│   └── container.rs # Container create/start/stop
└── security/        # Security features
    ├── caps.rs      # Linux capabilities
    ├── seccomp.rs   # Seccomp syscall filtering
    ├── apparmor.rs  # AppArmor profiles
    └── selinux.rs   # SELinux contexts
```

### Container Lifecycle

```
create() → configure namespaces → pivot_root → apply security → start()
    │                                                              │
    ▼                                                              ▼
Container      ←─────── cgroups attach ────────────▶         Exec init
Created                                                       process
    │                                                              │
    ▼                                                              ▼
  stop() ──────────────▶ signal SIGTERM/SIGKILL ──────────▶   Cleanup
```

## Image Builder (`bock-runtime`)

### Flow

```
Bockfile.yaml → Parser → Stage Resolution → Step Execution → Layer Creation → OCI Image
```

### Key Components

- **Bockfile** - YAML spec with stages, steps, runtime config
- **Builder** - Executes build process with caching
- **CacheManager** - Layer cache with pruning

## Networking (`bock-network`)

### Modes

- **Bridge** - Default NAT networking
- **Host** - Share host network
- **Macvlan** - Direct L2 connectivity
- **IPvlan** - L3 connectivity without MAC overhead
- **None** - No networking

### Components

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Container   │     │   veth pair  │     │    Bridge    │
│   Network    │◄───►│  (vethXXX)   │◄───►│   (bock0)    │
│   Namespace  │     │              │     │              │
└──────────────┘     └──────────────┘     └──────────────┘
                                                 │
                                                 ▼
                                          ┌──────────────┐
                                          │   iptables   │
                                          │   (NAT/FW)   │
                                          └──────────────┘
```

## Security Model

### Defense in Depth

1. **Namespaces** - Isolation (PID, NET, MNT, UTS, IPC, USER)
2. **Cgroups** - Resource limits
3. **Capabilities** - Minimal privilege set
4. **Seccomp** - Syscall filtering
5. **AppArmor/SELinux** - MAC policies
6. **User Namespaces** - UID/GID remapping for rootless

### Default Security Profile

- Drop all capabilities except minimal set
- Block dangerous syscalls (mount, ptrace, etc.)
- Enable `no_new_privs`
- Isolate all namespaces

## Unsafe Code

All `unsafe` blocks are documented with safety invariants. Key areas:

| Module | Usage | Safety Invariant |
|--------|-------|------------------|
| `netns.rs` | `setns()` | FD is valid netns, single-threaded context |
| `stdio.rs` | `pipe()`, `close()` | Valid FDs, proper ownership |
| `pty.rs` | `ioctl()` | Valid master FD, correct ioctl cmd |
| `console.rs` | `sendmsg()/recvmsg()` | Valid socket FD, SCM_RIGHTS semantics |
| `pivot.rs` | `pivot_root()` | Paths exist, mount namespace isolated |
| `init.rs` | Signal handlers | Single-threaded at install time |
| `process.rs` | `fork()` | Parent/child handling correct |
| `userns.rs` | `getuid()/getgid()` | Always safe syscalls |

## Configuration

### Environment Variables

| Variable | Description |
|----------|-------------|
| `BOCK_ROOT` | Runtime state directory (default: `/var/lib/bock`) |
| `BOCK_LOG` | Log level (trace, debug, info, warn, error) |
| `BOCK_REGISTRY_*_USERNAME` | Registry credentials |
| `BOCK_REGISTRY_*_PASSWORD` | Registry credentials |

### Paths

- Runtime: `/var/lib/bock/`
- Images: `/var/lib/bock/images/`
- Containers: `/var/lib/bock/containers/`
- Cache: `~/.cache/bock/`
- Credentials: `~/.bock/credentials.json`
