# Bock API Reference

## bock-common

### Error Types

```rust
/// Main error type for all Bock operations.
pub enum BockError {
    Io(std::io::Error),
    Config { message: String },
    Internal { message: String },
    Network { message: String },
    Registry { message: String },
    Unsupported { feature: String },
}

pub type BockResult<T> = Result<T, BockError>;
```

---

## bock-image

### ImageStore

Local image storage with OCI format support.

```rust
impl ImageStore {
    /// Create a new image store at the given path.
    pub fn new(root: impl Into<PathBuf>) -> BockResult<Self>;
    
    /// Save an image to the store.
    pub fn save(
        &mut self,
        reference: &str,
        manifest_bytes: &[u8],
        config_bytes: &[u8],
        layers: &[(String, Vec<u8>)],
    ) -> BockResult<StoredImage>;
    
    /// Load an image from the store.
    pub fn load(&self, reference: &str) -> BockResult<Option<StoredImage>>;
    
    /// List all stored images.
    pub fn list(&self) -> BockResult<Vec<StoredImage>>;
    
    /// Delete an image.
    pub fn delete(&mut self, reference: &str) -> BockResult<bool>;
    
    /// Extract image layers to a directory.
    pub fn extract_layers(&self, image: &StoredImage, dest: &Path) -> BockResult<()>;
    
    /// Garbage collect unused blobs.
    pub fn gc(&mut self) -> BockResult<u64>;
    
    /// Store a blob and return its digest.
    pub fn store_blob(&self, data: &[u8]) -> BockResult<String>;
    
    /// Get a blob by digest.
    pub fn get_blob(&self, digest: &str) -> BockResult<Option<Vec<u8>>>;
    
    /// Check if a blob exists.
    pub fn has_blob(&self, digest: &str) -> bool;
}
```

### StoredImage

```rust
pub struct StoredImage {
    pub reference: String,
    pub digest: String,
    pub config_digest: String,
    pub layers: Vec<String>,
    pub size: u64,
    pub created: Option<String>,
    pub architecture: String,
    pub os: String,
}
```

### RegistryClient

Client for pulling images from OCI registries.

```rust
impl RegistryClient {
    /// Create a new registry client.
    pub fn new(base_url: impl Into<String>) -> Self;
    
    /// Create a Docker Hub client.
    pub fn docker_hub() -> Self;
    
    /// Get an image manifest.
    pub async fn get_manifest(&mut self, name: &str, reference: &str) -> BockResult<String>;
    
    /// Get a blob by digest.
    pub async fn get_blob(&mut self, name: &str, digest: &str) -> BockResult<Vec<u8>>;
}
```

### Credentials

```rust
/// A registry credential.
pub struct Credential {
    pub registry: String,
    pub username: String,
    pub password: Option<String>,
    pub identity_token: Option<String>,
    pub email: Option<String>,
}

impl Credential {
    /// Create a new credential.
    pub fn new(registry: &str, username: &str, password: &str) -> Self;
    
    /// Create with OAuth token.
    pub fn with_token(registry: &str, token: &str) -> Self;
    
    /// Encode as Docker base64 auth.
    pub fn to_docker_auth(&self) -> String;
    
    /// Decode from Docker base64 auth.
    pub fn from_docker_auth(registry: &str, auth: &str) -> BockResult<Self>;
}

/// Trait for credential storage backends.
pub trait CredentialStore: Send + Sync {
    fn get(&self, registry: &str) -> BockResult<Option<Credential>>;
    fn store(&mut self, credential: Credential) -> BockResult<()>;
    fn delete(&mut self, registry: &str) -> BockResult<bool>;
    fn list(&self) -> BockResult<Vec<String>>;
    fn clear(&mut self) -> BockResult<()>;
    fn name(&self) -> &'static str;
}

/// File-based credential store (Docker config.json compatible).
pub struct FileCredentialStore { ... }

/// Password-store (pass) integration.
pub struct PassCredentialStore { ... }

/// Environment variable fallback.
pub struct EnvCredentialStore { ... }

/// Multi-backend credential manager.
pub struct CredentialManager { ... }
```

---

## bock-runtime

### Bockfile

```rust
/// Parsed Bockfile specification.
pub struct Bockfile {
    pub version: String,
    pub from: String,
    pub metadata: Metadata,
    pub args: HashMap<String, String>,
    pub stages: Vec<Stage>,
    pub runtime: RuntimeConfig,
    pub security: SecurityConfig,
}

impl Bockfile {
    /// Parse from YAML string.
    pub fn from_yaml(yaml: &str) -> BockResult<Self>;
    
    /// Parse from file path.
    pub fn from_file(path: &Path) -> BockResult<Self>;
}
```

### Builder

```rust
/// Image builder.
pub struct Builder { ... }

impl Builder {
    /// Create a new builder.
    pub fn new(bockfile: Bockfile, context: PathBuf, tag: String) -> Self;
    
    /// Create with options.
    pub fn with_options(
        bockfile: Bockfile,
        context: PathBuf,
        tag: String,
        options: BuildOptions,
    ) -> Self;
    
    /// Build the image.
    pub async fn build(&self) -> BockResult<BuiltImage>;
}

pub struct BuildOptions {
    pub args: HashMap<String, String>,
    pub no_cache: bool,
    pub target: Option<String>,
    pub output: Option<PathBuf>,
}

pub struct BuiltImage {
    pub digest: String,
    pub tag: String,
    pub layers: usize,
    pub size: u64,
}
```

### CacheManager

```rust
impl CacheManager {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self;
    pub fn has(&self, key: &str) -> bool;
    pub fn get(&self, key: &str) -> Option<PathBuf>;
    pub fn store(&mut self, key: &str, layer_path: &Path) -> BockResult<()>;
    pub fn prune(&mut self, max_age_days: u64) -> BockResult<u64>;
    pub fn clear(&mut self) -> BockResult<u64>;
    pub fn list(&self) -> Vec<CacheInfo>;
    pub fn total_size(&self) -> u64;
}
```

---

## bock-network

### Network Namespace Operations

```rust
/// Create a new network namespace.
pub fn create_netns(name: &str) -> BockResult<()>;

/// Delete a network namespace.
pub fn delete_netns(name: &str) -> BockResult<()>;

/// Enter a network namespace by name.
pub fn enter_netns(name: &str) -> BockResult<()>;

/// Enter a network namespace by PID.
pub fn enter_netns_by_pid(pid: u32) -> BockResult<()>;

/// List all network namespaces.
pub fn list_netns() -> BockResult<Vec<String>>;
```

### Bridge Networking

```rust
impl Bridge {
    pub fn new(name: &str) -> BockResult<Self>;
    pub fn add_address(&self, addr: &str) -> BockResult<()>;
    pub fn up(&self) -> BockResult<()>;
    pub fn delete(&self) -> BockResult<()>;
}
```

### Veth Pairs

```rust
impl VethPair {
    pub fn new(name1: &str, name2: &str) -> BockResult<Self>;
    pub fn move_to_namespace(&self, ns: &str) -> BockResult<()>;
}
```

---

## bock (Runtime)

### Container

```rust
pub struct Container {
    pub id: String,
    pub config: ContainerConfig,
    pub state: ContainerState,
}

pub fn create_container(config: ContainerConfig) -> BockResult<Container>;
```

### Cgroup Management

```rust
impl CgroupManager {
    pub fn new(name: &str) -> BockResult<Self>;
    pub fn apply_resources(&self, resources: &Resources) -> BockResult<()>;
    pub fn add_process(&self, pid: u32) -> BockResult<()>;
    pub fn cleanup(&self) -> BockResult<()>;
}
```

### Security

```rust
/// Apply capability restrictions.
impl CapabilitySet {
    pub fn apply(&self) -> BockResult<()>;
}

/// Apply seccomp filter.
impl SeccompFilter {
    pub fn apply(&self) -> BockResult<()>;
}
```
