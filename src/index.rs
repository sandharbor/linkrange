use crate::{
    metadata, parser, Diagnostic, FileInfo, FrontmatterField, IndexOptions, Metrics, Symlinks,
};
use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Stamp {
    size: u64,
    modified: i128,
    changed: i128,
    device: u64,
    inode: u64,
}
impl Stamp {
    fn read(path: &Path) -> Result<Self> {
        let m = fs::metadata(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                size: m.len(),
                modified: m.mtime() as i128 * 1_000_000_000 + m.mtime_nsec() as i128,
                changed: m.ctime() as i128 * 1_000_000_000 + m.ctime_nsec() as i128,
                device: m.dev(),
                inode: m.ino(),
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {
                size: m.len(),
                modified: m
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos() as i128,
                changed: 0,
                device: 0,
                inode: 0,
            })
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Record {
    pub stamp: Stamp,
    pub file: FileInfo,
    pub scan: parser::ScanResult,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Serialize, Deserialize)]
struct Cache {
    version: String,
    fields: Vec<FrontmatterField>,
    files: BTreeMap<String, Record>,
}

#[derive(Serialize)]
struct CacheView<'a> {
    version: &'a str,
    fields: &'a [FrontmatterField],
    files: &'a BTreeMap<String, Record>,
}

#[derive(Default, PartialEq, Eq)]
struct Inventory {
    files: BTreeMap<String, Stamp>,
    directories: BTreeSet<String>,
    aliases: BTreeMap<String, String>,
    diagnostics: Vec<Diagnostic>,
    incomplete: bool,
}

pub(crate) struct Index {
    pub files: Vec<Record>,
    pub directories: BTreeSet<String>,
    pub aliases: BTreeMap<String, String>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
    pub metrics: Metrics,
}

fn fail_or_record(
    options: &IndexOptions,
    inventory: &mut Inventory,
    path: &str,
    error: impl std::fmt::Display,
) -> Result<()> {
    if !options.best_effort {
        anyhow::bail!("Could not index {path}: {error}; previous cache preserved");
    }
    inventory.incomplete = true;
    inventory
        .diagnostics
        .push(Diagnostic::new(path, "indexingFailed", error));
    Ok(())
}

fn relative(root: &Path, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(root)?
        .to_str()
        .context("Source path is not UTF-8")?
        .replace('\\', "/"))
}

fn inventory(root: &Path, options: &IndexOptions) -> Result<Inventory> {
    fn walk(
        root: &Path,
        dir: &Path,
        options: &IndexOptions,
        result: &mut Inventory,
        active: &mut BTreeSet<PathBuf>,
    ) -> Result<()> {
        let relative_dir = relative(root, dir)?;
        if !result.directories.insert(relative_dir.clone()) {
            return Ok(());
        }
        active.insert(dir.into());
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) => {
                fail_or_record(options, result, &relative_dir, error)?;
                return Ok(());
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    fail_or_record(options, result, &relative_dir, error)?;
                    continue;
                }
            };
            if !options.include_hidden && entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            let name = match relative(root, &path) {
                Ok(name) => name,
                Err(error) => {
                    fail_or_record(options, result, &path.to_string_lossy(), error)?;
                    continue;
                }
            };
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(error) => {
                    fail_or_record(options, result, &name, error)?;
                    continue;
                }
            };
            let target = if kind.is_symlink() {
                if options.symlinks == Symlinks::Skip {
                    result.diagnostics.push(Diagnostic::new(
                        &name,
                        "symlinkSkipped",
                        "Following symlinks is disabled",
                    ));
                    continue;
                }
                let target = match path.canonicalize() {
                    Ok(path) => path,
                    Err(error) => {
                        fail_or_record(options, result, &name, error)?;
                        continue;
                    }
                };
                if !target.starts_with(root) {
                    result.diagnostics.push(Diagnostic::new(
                        &name,
                        "symlinkOutsideRoot",
                        "Target is outside the source root",
                    ));
                    continue;
                }
                let target_name = relative(root, &target)?;
                if !options.include_hidden
                    && target_name.split('/').any(|part| part.starts_with('.'))
                {
                    result.diagnostics.push(Diagnostic::new(
                        &name,
                        "symlinkHiddenTarget",
                        "Target is excluded by hidden-file policy",
                    ));
                    continue;
                }
                result.aliases.insert(name.clone(), target_name);
                if active.contains(&target) {
                    result.diagnostics.push(Diagnostic::new(
                        &name,
                        "symlinkCycle",
                        "Directory ancestor already being scanned",
                    ));
                    continue;
                }
                target
            } else {
                path
            };
            let is_directory = if kind.is_symlink() {
                target.is_dir()
            } else {
                kind.is_dir()
            };
            let is_file = if kind.is_symlink() {
                target.is_file()
            } else {
                kind.is_file()
            };
            if is_directory {
                walk(root, &target, options, result, active)?;
            } else if is_file
                && parser::is_supported_source_extension(
                    &target
                        .extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase(),
                )
            {
                match Stamp::read(&target) {
                    Ok(stamp) => {
                        result.files.insert(relative(root, &target)?, stamp);
                    }
                    Err(error) => fail_or_record(options, result, &name, error)?,
                }
            }
        }
        active.remove(dir);
        Ok(())
    }
    let mut result = Inventory::default();
    walk(root, root, options, &mut result, &mut BTreeSet::new())?;
    result
        .diagnostics
        .sort_by(|a, b| (&a.path, &a.code).cmp(&(&b.path, &b.code)));
    Ok(result)
}

pub(crate) fn load(root: &Path, options: &IndexOptions) -> Result<Index> {
    let start = Instant::now();
    for field in &options.frontmatter {
        anyhow::ensure!(
            !field.key.is_empty() && !field.substring.as_deref().unwrap_or(&field.key).is_empty(),
            "Frontmatter keys and prefilters must be nonempty"
        );
    }
    let mut fields = options.frontmatter.clone();
    fields.sort_by(|a, b| (&a.key, &a.substring).cmp(&(&b.key, &b.substring)));
    fields.dedup();
    let version = format!(
        "{:x}",
        Sha256::digest(
            concat!(
                env!("CARGO_PKG_VERSION"),
                include_str!("parser.rs"),
                include_str!("links.rs"),
                include_str!("sources.rs"),
                include_str!("markup.rs"),
                include_str!("metadata.rs"),
                include_str!("index.rs")
            )
            .as_bytes()
        )
    );
    let mut metrics = Metrics::default();
    let mut lock = None;
    let filename = if options.no_cache {
        None
    } else {
        let dir = options
            .cache_directory
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("linkrange-cache"));
        fs::create_dir_all(&dir)?;
        let dir = dir.canonicalize()?;
        anyhow::ensure!(
            !dir.starts_with(root),
            "Cache directory must be outside the source root"
        );
        let key = format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes()));
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(dir.join(format!("{key}.lock")))?;
        file.lock_exclusive()?;
        lock = Some(file);
        Some(dir.join(format!("{key}.json")))
    };
    let mut previous = if options.rebuild {
        None
    } else {
        filename
            .as_ref()
            .and_then(|path| File::open(path).ok())
            .and_then(|file| serde_json::from_reader::<_, Cache>(BufReader::new(file)).ok())
            .filter(|cache| cache.version == version)
    };
    metrics.cache_rebuilt = previous.is_none();
    let same_fields = previous
        .as_ref()
        .is_some_and(|cache| cache.fields == fields);
    metrics
        .phases_ms
        .insert("cacheLoad".into(), start.elapsed().as_secs_f64() * 1000.0);
    let phase = Instant::now();
    let mut inventory = inventory(root, options)?;
    metrics
        .phases_ms
        .insert("inventory".into(), phase.elapsed().as_secs_f64() * 1000.0);
    let phase = Instant::now();
    let mut next = BTreeMap::new();
    let mut link_parse_ms = 0.0;
    let mut metadata_ms = 0.0;
    let mut changed = !same_fields;
    for (name, stamp) in &inventory.files.clone() {
        let cached = previous.as_mut().and_then(|cache| cache.files.remove(name));
        let unchanged = cfg!(unix) && cached.as_ref().is_some_and(|entry| entry.stamp == *stamp);
        if unchanged && same_fields {
            next.insert(name.clone(), cached.unwrap());
            continue;
        }
        changed = true;
        let path = root.join(name);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                fail_or_record(options, &mut inventory, name, error)?;
                continue;
            }
        };
        metrics.files_read += 1;
        if Stamp::read(&path).ok().as_ref() != Some(stamp) {
            fail_or_record(
                options,
                &mut inventory,
                name,
                "Source changed while reading; retry",
            )?;
            continue;
        }
        let text_format = matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("md" | "html" | "svg" | "css" | "js" | "txt")
        );
        if text_format && std::str::from_utf8(&bytes).is_err() {
            fail_or_record(options, &mut inventory, name, "Text source is not UTF-8")?;
            continue;
        }
        let scan = if unchanged {
            cached.unwrap().scan
        } else {
            let timer = Instant::now();
            metrics.link_parses += 1;
            let scan = parser::scan_file(&path, root, &bytes);
            link_parse_ms += timer.elapsed().as_secs_f64() * 1000.0;
            scan
        };
        let timer = Instant::now();
        let (metadata, diagnostics, parsed) =
            if path.extension().is_some_and(|extension| extension == "md") {
                metadata::collect(name, std::str::from_utf8(&bytes).unwrap_or(""), &fields)
            } else {
                (BTreeMap::new(), Vec::new(), 0)
            };
        metrics.yaml_parses += parsed;
        metadata_ms += timer.elapsed().as_secs_f64() * 1000.0;
        let file = FileInfo {
            path: name.clone(),
            format: scan.source_file.file_type.clone(),
            title: scan.source_file.title.clone(),
            directory: scan.source_file.directory.clone(),
            digest: format!("{:x}", Sha256::digest(&bytes)),
            size: bytes.len() as u64,
            metadata,
        };
        next.insert(
            name.clone(),
            Record {
                stamp: stamp.clone(),
                file,
                scan,
                diagnostics,
            },
        );
    }
    changed |= previous
        .as_ref()
        .is_some_and(|cache| !cache.files.is_empty());
    metrics.phases_ms.insert(
        "readAndParse".into(),
        phase.elapsed().as_secs_f64() * 1000.0,
    );
    metrics
        .phases_ms
        .insert("linkParsing".into(), link_parse_ms);
    metrics.phases_ms.insert("frontmatter".into(), metadata_ms);
    let phase = Instant::now();
    if changed && !inventory.incomplete {
        let final_inventory = self::inventory(root, options)?;
        if final_inventory != inventory {
            anyhow::bail!("Source files changed while indexing; retry; previous cache preserved");
        }
        if let Some(filename) = &filename {
            let staged = filename.with_extension(format!("{}.tmp", std::process::id()));
            let mut writer = BufWriter::new(File::create(&staged)?);
            serde_json::to_writer(
                &mut writer,
                &CacheView {
                    version: &version,
                    fields: &fields,
                    files: &next,
                },
            )?;
            writer.flush()?;
            writer.get_ref().sync_all()?;
            fs::rename(&staged, filename)?;
        }
    }
    metrics.cache_bytes = filename
        .as_ref()
        .and_then(|path| fs::metadata(path).ok())
        .map_or(0, |metadata| metadata.len());
    metrics.indexed_files = next.len();
    metrics
        .phases_ms
        .insert("cacheWrite".into(), phase.elapsed().as_secs_f64() * 1000.0);
    drop(lock);
    Ok(Index {
        files: next.into_values().collect(),
        directories: inventory.directories,
        aliases: inventory.aliases,
        diagnostics: inventory.diagnostics,
        complete: !inventory.incomplete,
        metrics,
    })
}
