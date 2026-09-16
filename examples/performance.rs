//! Deterministic corpus generator and JSON-lines diagnostic. Never run by cargo test.
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use linkrange::{Depths, FrontmatterField, Graph, IndexOptions, Query, Start};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Generate {
        #[arg(long)]
        directory: PathBuf,
        #[arg(long, default_value_t = 500_000)]
        files: usize,
    },
    Measure {
        #[arg(long)]
        directory: PathBuf,
        #[arg(long, default_value_t = 3)]
        repeats: usize,
        #[arg(long, default_value = "all")]
        scenario: String,
    },
}
fn name(id: usize) -> String {
    format!("{:04}/note-{id:06}.md", id / 1000)
}
fn body(id: usize, count: usize) -> String {
    let header = match id % 10 {
        0 => format!("---\ntag: group-{}\nsensitive: true\n---\n", id % 17),
        1 => "---\nunrelated: [malformed\n---\n".into(),
        2 => "---\ntag: other\n---\n".into(),
        _ => String::new(),
    };
    let mut text = format!(
        "{header}# Note {id}\n\nA deterministic context graph benchmark. {}\n",
        "ordinary prose ".repeat(id % 32)
    );
    for child in (id * 3 + 1)..=(id * 3 + 3) {
        if child < count {
            // Three syntaxes share the same deterministic physical graph.
            let target = name(child);
            match child % 3 {
                0 => text.push_str(&format!("[[{target}|related]]\n")),
                1 => text.push_str(&format!("[related](/{target})\n")),
                _ => text.push_str(&format!("<a href=\"/{target}\">related</a>\n")),
            }
        }
    }
    text.push_str("`[[example]]`\n<!-- <img src=\"example.svg\"> -->\n");
    text
}
fn generate(directory: &Path, count: usize) -> Result<()> {
    anyhow::ensure!(count > 0, "File count must be positive");
    fs::create_dir(directory)
        .context("Use a new corpus directory; existing data is never overwritten")?;
    let root = directory.join("source");
    fs::create_dir(&root)?;
    let timer = Instant::now();
    let mut digest = Sha256::new();
    let mut bytes = 0;
    for id in 0..count {
        let path = name(id);
        let text = body(id, count);
        if id % 1000 == 0 {
            fs::create_dir(root.join(format!("{:04}", id / 1000)))?;
        }
        fs::write(root.join(&path), &text)?;
        digest.update(path.as_bytes());
        digest.update(text.as_bytes());
        bytes += text.len();
        if id > 0 && id % 50_000 == 0 {
            eprintln!("Generated {id}/{count} files");
        }
    }
    let manifest = json!({"schemaVersion":1,"generator":"ternary-notes-v1","files":count,"sourceBytes":bytes,"sha256":format!("{:x}",digest.finalize()),"seed":0});
    fs::write(
        directory.join("corpus.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "{}",
        json!({"event":"generated","corpus":manifest,"elapsedMs":timer.elapsed().as_secs_f64()*1000.0})
    );
    Ok(())
}
#[cfg(unix)]
fn peak_rss() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // getrusage initializes the supplied record on success; it does not retain it.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    let raw = unsafe { usage.assume_init() }.ru_maxrss as u64;
    Some(if cfg!(target_os = "macos") {
        raw
    } else {
        raw * 1024
    })
}
#[cfg(not(unix))]
fn peak_rss() -> Option<u64> {
    None
}
fn measure(directory: &Path, repeats: usize, selected: &str) -> Result<()> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("corpus.json"))?)?;
    anyhow::ensure!(
        manifest["generator"] == "ternary-notes-v1",
        "Unknown corpus generator"
    );
    let count = manifest["files"].as_u64().context("Missing file count")? as usize;
    let root = directory.join("source");
    let options = IndexOptions {
        cache_directory: Some(directory.join("cache")),
        frontmatter: vec![FrontmatterField {
            key: "sensitive".into(),
            substring: None,
        }],
        ..Default::default()
    };
    let cases = [
        "cold",
        "warm",
        "incremental",
        "metadata-change",
        "no-cache",
        "wide",
    ];
    anyhow::ensure!(
        selected == "all" || cases.contains(&selected),
        "Unknown scenario"
    );
    anyhow::ensure!(repeats > 0, "Repeats must be positive");
    // Prime outside measurement. Warm means a persisted index, not an in-memory Graph.
    drop(Graph::open(&root, &options)?);
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());
    for scenario in cases
        .into_iter()
        .filter(|case| selected == "all" || *case == selected)
    {
        for sample in 0..repeats {
            let mut config = options.clone();
            let hops = if scenario == "wide" { 8 } else { 3 };
            config.rebuild = scenario == "cold";
            config.no_cache = scenario == "no-cache";
            if scenario == "metadata-change" {
                config.frontmatter.push(FrontmatterField {
                    key: "tag".into(),
                    substring: None,
                });
            }
            let edited = root.join(name(count - 1));
            let original = if scenario == "incremental" {
                let original = fs::read(&edited)?;
                let mut edited_bytes = original.clone();
                edited_bytes.extend_from_slice(b"\ncontent-only edit\n");
                fs::write(&edited, edited_bytes)?;
                Some(original)
            } else {
                None
            };
            let timer = Instant::now();
            let graph = Graph::open(&root, &config)?;
            let opened = timer.elapsed().as_secs_f64() * 1000.0;
            let response = graph.query(&Query {
                starts: vec![Start {
                    path: name(0),
                    depths: None,
                }],
                depths: Depths {
                    outlinks: hops,
                    inlinks: 0,
                },
                adjacency: true,
                ..Default::default()
            })?;
            let expected = ((3usize.pow(hops + 1) - 1) / 2).min(count);
            anyhow::ensure!(
                response.complete && response.nodes.len() == expected,
                "Traversal correctness failure: expected {expected}, got {}",
                response.nodes.len()
            );
            let mut semantic = serde_json::to_value(&response)?;
            semantic.as_object_mut().unwrap().remove("metrics");
            let serialize = Instant::now();
            let output = serde_json::to_vec(&semantic)?;
            let serialization_ms = serialize.elapsed().as_secs_f64() * 1000.0;
            println!(
                "{}",
                json!({"schemaVersion":1,"event":"measurement","scenario":scenario,"sample":sample,"revision":revision,
                "platform":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"threads":std::thread::available_parallelism().map(|n|n.get()).ok()},
                "corpus":manifest,"totalMs":timer.elapsed().as_secs_f64()*1000.0,"openMs":opened,"serializationMs":serialization_ms,
                "metrics":response.metrics,"peakProcessRssBytes":peak_rss(),"responseBytes":output.len(),"nodes":response.nodes.len(),"edges":response.edges.len(),
                "correct":true,"semanticSha256":format!("{:x}",Sha256::digest(&output))})
            );
            drop(graph);
            if let Some(original) = original {
                fs::write(&edited, original)?;
                drop(Graph::open(&root, &options)?);
            }
            if scenario == "metadata-change" {
                drop(Graph::open(&root, &options)?);
            }
        }
    }
    Ok(())
}
fn main() -> Result<()> {
    match Args::parse().command {
        Command::Generate { directory, files } => generate(&directory, files),
        Command::Measure {
            directory,
            repeats,
            scenario,
        } => measure(&directory, repeats, &scenario),
    }
}
