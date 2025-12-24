//! Image builder.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use bock_common::BockResult;
use sha2::{Digest, Sha256};

use crate::bockfile_v2::{AddStep, Bockfile, CopyStep, EnvStep, RunStep, Stage, Step};
use crate::cache::CacheManager;

/// Image builder.
pub struct Builder {
    /// Bockfile specification.
    bockfile: Bockfile,
    /// Build context directory.
    context: PathBuf,
    /// Target tag.
    tag: String,
    /// Build arguments.
    build_args: HashMap<String, String>,
    /// Cache manager.
    cache: CacheManager,
    /// No cache flag.
    no_cache: bool,
}

/// Built image result.
#[derive(Debug, Clone)]
pub struct BuiltImage {
    /// Image digest.
    pub digest: String,
    /// Image tag.
    pub tag: String,
    /// Number of layers.
    pub layers: usize,
    /// Total size in bytes.
    pub size: u64,
}

/// Build options.
#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    /// Build arguments.
    pub args: HashMap<String, String>,
    /// Disable cache.
    pub no_cache: bool,
    /// Target stage (for multi-stage).
    pub target: Option<String>,
    /// Output directory for OCI image.
    pub output: Option<PathBuf>,
}

impl Builder {
    /// Create a new builder.
    pub fn new(bockfile: Bockfile, context: PathBuf, tag: String) -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("bock")
            .join("build-cache");

        Self {
            bockfile,
            context,
            tag,
            build_args: HashMap::new(),
            cache: CacheManager::new(cache_dir),
            no_cache: false,
        }
    }

    /// Create builder with options.
    pub fn with_options(
        bockfile: Bockfile,
        context: PathBuf,
        tag: String,
        options: BuildOptions,
    ) -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("bock")
            .join("build-cache");

        Self {
            bockfile,
            context,
            tag,
            build_args: options.args,
            cache: CacheManager::new(cache_dir),
            no_cache: options.no_cache,
        }
    }

    /// Build the image.
    pub async fn build(&self) -> BockResult<BuiltImage> {
        tracing::info!(tag = %self.tag, "Building image");

        // Create build directory
        let build_dir = tempfile::tempdir().map_err(|e| bock_common::BockError::Io(e))?;
        let rootfs = build_dir.path().join("rootfs");
        fs::create_dir_all(&rootfs)?;

        let mut layers = Vec::new();
        let mut current_env: HashMap<String, String> = self.bockfile.runtime.env.clone();
        let mut current_workdir = self
            .bockfile
            .runtime
            .workdir
            .clone()
            .unwrap_or_else(|| "/".to_string());
        let mut current_user = self.bockfile.security.user.clone();
        let mut current_entrypoint = self.bockfile.runtime.entrypoint.clone();
        let mut current_cmd = self.bockfile.runtime.cmd.clone();
        let mut current_exposed_ports = self.bockfile.runtime.ports.clone();
        let mut current_volumes = self.bockfile.runtime.volumes.clone();
        let mut current_labels = self.bockfile.metadata.labels.clone();

        // Build dependency graph and execute stages
        let stages = self.resolve_stages()?;

        for stage in &stages {
            tracing::info!(stage = %stage.name, "Building stage");

            for step in &stage.steps {
                let layer_digest = self
                    .execute_step(
                        step,
                        &rootfs,
                        &mut current_env,
                        &mut current_workdir,
                        &mut current_user,
                        &mut current_entrypoint,
                        &mut current_cmd,
                        &mut current_exposed_ports,
                        &mut current_volumes,
                        &mut current_labels,
                    )
                    .await?;

                if let Some(digest) = layer_digest {
                    layers.push(digest);
                }
            }
        }

        // Calculate final digest
        let digest = self.calculate_image_digest(&layers);

        // Generate OCI image config
        self.generate_oci_image(
            &rootfs,
            &layers,
            &current_env,
            &current_workdir,
            &current_entrypoint,
            &current_cmd,
            &current_exposed_ports,
            &current_volumes,
            &current_labels,
        )?;

        let size = self.calculate_size(&rootfs)?;

        tracing::info!(
            tag = %self.tag,
            digest = %digest,
            layers = layers.len(),
            size = size,
            "Image built successfully"
        );

        Ok(BuiltImage {
            digest,
            tag: self.tag.clone(),
            layers: layers.len(),
            size,
        })
    }

    /// Resolve stage execution order based on dependencies.
    fn resolve_stages(&self) -> BockResult<Vec<Stage>> {
        // If no stages defined, create a default one
        if self.bockfile.stages.is_empty() {
            return Ok(vec![Stage {
                name: "default".to_string(),
                alias: None,
                from: None,
                depends: Vec::new(),
                steps: Vec::new(),
                security: None,
                cache: None,
                workdir: None,
                env: HashMap::new(),
            }]);
        }

        // Simple topological sort
        let mut resolved = Vec::new();
        let mut remaining: Vec<_> = self.bockfile.stages.iter().cloned().collect();

        while !remaining.is_empty() {
            let mut progressed = false;

            remaining.retain(|stage| {
                let deps_satisfied = stage
                    .depends
                    .iter()
                    .all(|dep| resolved.iter().any(|s: &Stage| &s.name == dep));

                if deps_satisfied {
                    resolved.push(stage.clone());
                    progressed = true;
                    false
                } else {
                    true
                }
            });

            if !progressed && !remaining.is_empty() {
                return Err(bock_common::BockError::Config {
                    message: format!(
                        "Circular dependency in stages: {:?}",
                        remaining.iter().map(|s| &s.name).collect::<Vec<_>>()
                    ),
                });
            }
        }

        Ok(resolved)
    }

    /// Execute a build step.
    async fn execute_step(
        &self,
        step: &Step,
        rootfs: &Path,
        env: &mut HashMap<String, String>,
        workdir: &mut String,
        user: &mut Option<String>,
        entrypoint: &mut Option<Vec<String>>,
        cmd: &mut Option<Vec<String>>,
        exposed_ports: &mut Vec<String>,
        volumes: &mut Vec<String>,
        labels: &mut HashMap<String, String>,
    ) -> BockResult<Option<String>> {
        match step {
            Step::Run(run) => self.execute_run(run, rootfs, env, workdir).await,
            Step::Copy(copy) => self.execute_copy(copy, rootfs).await,
            Step::Add(add) => self.execute_add(add, rootfs).await,
            Step::User(u) => {
                *user = Some(u.clone());
                Ok(None)
            }
            Step::Workdir(w) => {
                *workdir = w.clone();
                // Create workdir
                let dir = rootfs.join(w.trim_start_matches('/'));
                fs::create_dir_all(&dir)?;
                Ok(None)
            }
            Step::Env(e) => {
                match e {
                    EnvStep::Single { key, value } => {
                        env.insert(key.clone(), value.clone());
                    }
                    EnvStep::Multiple(map) => {
                        env.extend(map.clone());
                    }
                }
                Ok(None)
            }
            Step::Entrypoint(ep) => {
                *entrypoint = Some(ep.clone());
                Ok(None)
            }
            Step::Cmd(c) => {
                *cmd = Some(c.clone());
                Ok(None)
            }
            Step::Expose(e) => {
                let port_str = match e {
                    crate::bockfile_v2::ExposeStep::Port(p) => p.to_string(),
                    crate::bockfile_v2::ExposeStep::Detailed { port, protocol } => {
                        if let Some(proto) = protocol {
                            format!("{}/{}", port, proto)
                        } else {
                            port.to_string()
                        }
                    }
                };
                if !exposed_ports.contains(&port_str) {
                    exposed_ports.push(port_str);
                }
                Ok(None)
            }
            Step::Volume(v) => {
                if !volumes.contains(v) {
                    volumes.push(v.clone());
                }
                Ok(None)
            }
            Step::Label(l) => {
                labels.extend(l.clone());
                Ok(None)
            }
            Step::Shell(_) | Step::Healthcheck(_) => {
                // Shell and Healthcheck currently ignored for basic OCI config, but could be added
                Ok(None)
            }
        }
    }

    /// Execute a RUN step.
    async fn execute_run(
        &self,
        run: &RunStep,
        rootfs: &Path,
        env: &HashMap<String, String>,
        workdir: &str,
    ) -> BockResult<Option<String>> {
        let (command, run_workdir) = match run {
            RunStep::Simple(cmd) => (cmd.clone(), None),
            RunStep::Detailed { run, workdir, .. } => (run.clone(), workdir.clone()),
        };

        // Calculate cache key
        let cache_key = self.calculate_step_key(&command, env);

        // Check cache
        if !self.no_cache && self.cache.has(&cache_key) {
            tracing::debug!(key = %cache_key, "Using cached layer");
            return Ok(Some(cache_key));
        }

        tracing::debug!(cmd = %command, "Executing RUN step");

        // Substitute build args
        let cmd = self.substitute_args(&command);

        // For actual execution, we would use namespaces/chroot
        // For now, simulate with a simple command
        let wd = run_workdir.as_deref().unwrap_or(workdir);

        // Create the working directory in rootfs
        let full_workdir = rootfs.join(wd.trim_start_matches('/'));
        fs::create_dir_all(&full_workdir)?;

        // Simulate by writing a script
        let script_path = rootfs.join("tmp/build-script.sh");
        if let Some(parent) = script_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&script_path, format!("#!/bin/sh\ncd {} && {}", wd, cmd))?;

        tracing::info!(cmd = %cmd, workdir = %wd, "RUN step completed (simulated)");

        // Note: In a real implementation, we would store the layer in cache here
        // self.cache.store(&cache_key, &rootfs.to_path_buf())?;
        tracing::debug!(cache_key = %cache_key, "Layer would be cached");

        Ok(Some(cache_key))
    }

    /// Execute a COPY step.
    async fn execute_copy(&self, copy: &CopyStep, rootfs: &Path) -> BockResult<Option<String>> {
        let (sources, destination) = match copy {
            CopyStep::Simple { from, to } => (vec![from.clone()], to.clone()),
            CopyStep::Detailed { copy, to, .. } => (copy.clone(), to.clone()),
        };

        let dest = rootfs.join(destination.trim_start_matches('/'));
        fs::create_dir_all(&dest)?;

        // Calculate cache key based on source files
        let mut hasher = Sha256::new();

        for src in &sources {
            let src_path = self.context.join(src);

            if src_path.exists() {
                if src_path.is_file() {
                    let content = fs::read(&src_path)?;
                    hasher.update(&content);

                    let dest_file = dest.join(src_path.file_name().unwrap_or_default());
                    if let Some(p) = dest_file.parent() {
                        fs::create_dir_all(p)?;
                    }
                    fs::copy(&src_path, &dest_file)?;

                    tracing::debug!(src = %src, dest = %dest_file.display(), "Copied file");
                } else if src_path.is_dir() {
                    // Copy directory recursively
                    copy_dir_recursive(&src_path, &dest)?;
                    hasher.update(src.as_bytes());
                }
            } else {
                // Try glob pattern
                for entry in glob::glob(&self.context.join(src).to_string_lossy()).map_err(|e| {
                    bock_common::BockError::Config {
                        message: format!("Invalid glob pattern: {}", e),
                    }
                })? {
                    if let Ok(path) = entry {
                        if path.is_file() {
                            let content = fs::read(&path)?;
                            hasher.update(&content);

                            let dest_file = dest.join(path.file_name().unwrap_or_default());
                            if let Some(p) = dest_file.parent() {
                                fs::create_dir_all(p)?;
                            }
                            fs::copy(&path, &dest_file)?;
                        }
                    }
                }
            }
        }

        let digest = format!("sha256:{:x}", hasher.finalize());
        tracing::info!(dest = %destination, digest = %digest, "COPY step completed");

        Ok(Some(digest))
    }

    /// Execute an ADD step.
    async fn execute_add(&self, add: &AddStep, rootfs: &Path) -> BockResult<Option<String>> {
        let dest = rootfs.join(add.to.trim_start_matches('/'));
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        // Handle URL or local file
        let content = if add.add.starts_with("http://") || add.add.starts_with("https://") {
            tracing::info!(url = %add.add, "Downloading file");
            reqwest::get(&add.add)
                .await
                .map_err(|e| {
                    bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
                })?
                .bytes()
                .await
                .map_err(|e| {
                    bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
                })?
                .to_vec()
        } else {
            let src_path = self.context.join(&add.add);
            fs::read(&src_path)?
        };

        // Check checksum
        if let Some(expected) = &add.checksum {
            let digest = format!("sha256:{:x}", Sha256::digest(&content));
            if expected != &digest && expected != &format!("sha256:{}", digest) {
                return Err(bock_common::BockError::Config {
                    message: format!("Checksum mismatch. Expected {}, got {}", expected, digest),
                });
            }
        }

        // Auto-extract if it's an archive and extract is true
        if add.extract
            && (add.add.ends_with(".tar.gz")
                || add.add.ends_with(".tgz")
                || add.add.ends_with(".tar"))
        {
            // Extraction logic would go here (simplified for now)
            tracing::info!("Extracting archive to {}", dest.display());
            // For now just write the file to show it works
            fs::write(&dest, &content)?;
        } else {
            fs::write(&dest, &content)?;
        }

        // Calculate digest
        let digest = format!("sha256:{:x}", Sha256::digest(&content));
        Ok(Some(digest))
    }

    /// Substitute build arguments in a string.
    fn substitute_args(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Resolve bockfile args
        let resolved_args = self.bockfile.resolve_args();

        // Apply Bockfile args
        for (key, value) in &resolved_args {
            result = result.replace(&format!("${{{}}}", key), value);
            result = result.replace(&format!("${}", key), value);
        }

        // Apply build-time args overrides
        for (key, value) in &self.build_args {
            result = result.replace(&format!("${{{}}}", key), value);
            result = result.replace(&format!("${}", key), value);
        }

        result
    }

    /// Calculate a cache key for a step.
    fn calculate_step_key(&self, cmd: &str, env: &HashMap<String, String>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(cmd.as_bytes());

        // Include environment in hash
        let mut sorted_env: Vec<_> = env.iter().collect();
        sorted_env.sort_by_key(|(k, _)| *k);
        for (k, v) in sorted_env {
            hasher.update(k.as_bytes());
            hasher.update(v.as_bytes());
        }

        format!("{:x}", hasher.finalize())
    }

    /// Calculate final image digest.
    fn calculate_image_digest(&self, layers: &[String]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.tag.as_bytes());
        for layer in layers {
            hasher.update(layer.as_bytes());
        }
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Generate OCI image structure.
    fn generate_oci_image(
        &self,
        rootfs: &Path,
        layers: &[String],
        env: &HashMap<String, String>,
        workdir: &str,
        entrypoint: &Option<Vec<String>>,
        cmd: &Option<Vec<String>>,
        exposed_ports: &[String],
        volumes: &[String],
        labels: &HashMap<String, String>,
    ) -> BockResult<()> {
        // Create OCI layout
        let oci_dir = rootfs.parent().unwrap().join("oci");
        fs::create_dir_all(&oci_dir)?;

        // Write oci-layout
        fs::write(
            oci_dir.join("oci-layout"),
            r#"{"imageLayoutVersion":"1.0.0"}"#,
        )?;

        // Create blobs directory
        let blobs_dir = oci_dir.join("blobs").join("sha256");
        fs::create_dir_all(&blobs_dir)?;

        // Generate config
        let config = serde_json::json!({
            "architecture": std::env::consts::ARCH,
            "os": "linux",
            "config": {
                "Env": env.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>(),
                "Entrypoint": entrypoint,
                "Cmd": cmd,
                "WorkingDir": workdir,
                "ExposedPorts": exposed_ports.iter().map(|p| (p, serde_json::json!({}))).collect::<HashMap<_, _>>(),
                "Volumes": volumes.iter().map(|v| (v, serde_json::json!({}))).collect::<HashMap<_, _>>(),
                "Labels": labels,
            },
            "rootfs": {
                "type": "layers",
                "diff_ids": layers,
            }
        });

        let config_bytes =
            serde_json::to_vec_pretty(&config).map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to serialize config: {}", e),
            })?;

        let config_digest = format!("{:x}", Sha256::digest(&config_bytes));
        fs::write(blobs_dir.join(&config_digest), &config_bytes)?;

        tracing::debug!(path = %oci_dir.display(), "OCI image structure generated");
        Ok(())
    }

    /// Calculate total size of rootfs.
    fn calculate_size(&self, rootfs: &Path) -> BockResult<u64> {
        let mut total = 0;

        for entry in walkdir::WalkDir::new(rootfs) {
            if let Ok(e) = entry {
                if e.file_type().is_file() {
                    if let Ok(meta) = e.metadata() {
                        total += meta.len();
                    }
                }
            }
        }

        Ok(total)
    }
}

/// Copy a directory recursively.
fn copy_dir_recursive(src: &Path, dest: &Path) -> BockResult<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_args() {
        let bockfile = Bockfile {
            base: crate::bockfile_v2::BaseImage {
                from: "alpine".to_string(),
                alias: None,
                version: Some("1".to_string()),
                platform: None,
            },
            metadata: Default::default(),
            args: [(
                "VERSION".to_string(),
                crate::bockfile_v2::ArgValue::Simple("1.0".to_string()),
            )]
            .into_iter()
            .collect(),
            stages: Vec::new(),
            runtime: Default::default(),
            security: Default::default(),
            registry: None,
        };

        let builder = Builder::new(bockfile, PathBuf::from("."), "test".to_string());

        let result = builder.substitute_args("echo ${VERSION}");
        assert_eq!(result, "echo 1.0");
    }
}
