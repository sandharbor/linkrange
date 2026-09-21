//! Deterministic filesystem corpus and process-level CLI diagnostic, outside cargo test.
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use linkrange::{Depths, FrontmatterField, IndexOptions, Query, Request, Response, Start};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command as Process,
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
        #[arg(long, default_value_t = 3)]
        branching: usize,
        #[arg(long, default_value_t = 1000)]
        directory_size: usize,
        #[arg(long, default_value_t = 10)]
        frontmatter_every: usize,
        #[arg(long, default_value_t = 32)]
        prose_words: usize,
        #[arg(long, default_value_t = 0)]
        seed: u64,
    },
    Measure {
        #[arg(long)]
        directory: PathBuf,
        #[arg(long, default_value_t = 3)]
        repeats: usize,
        #[arg(long, default_value = "all")]
        scenario: String,
        #[arg(long)]
        cli: Option<PathBuf>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Corpus {
    schema_version: u32,
    generator: String,
    files: usize,
    branching: usize,
    directory_size: usize,
    frontmatter_every: usize,
    prose_words: usize,
    seed: u64,
    source_bytes: usize,
    sha256: String,
}
impl Corpus {
    fn extension(&self, id: usize) -> &str {
        match id % 25 {
            20 => "html",
            21 => "svg",
            22 => "excalidraw.md",
            23 if id * self.branching + 1 >= self.files => "png",
            _ => "md",
        }
    }
    fn name(&self, id: usize) -> String {
        let title = if id % self.directory_size == self.directory_size - 1 {
            "shared".into()
        } else {
            format!("note-{id:06}")
        };
        format!(
            "{:04}/{title}.{}",
            id / self.directory_size,
            self.extension(id)
        )
    }
    fn body(&self, id: usize) -> Vec<u8> {
        let extension = self.extension(id);
        if extension == "png" {
            return b"\x89PNG\r\n\x1a\nbenchmark asset".to_vec();
        }
        let words = (id as u64).wrapping_add(self.seed).wrapping_mul(1103515245) as usize
            % self.prose_words.max(1);
        let prose = "ordinary prose ".repeat(words);
        let children = (id * self.branching + 1..=id * self.branching + self.branching)
            .filter(|child| *child < self.files);
        if extension == "html" || extension == "svg" {
            let mut body = if extension == "html" {
                format!("<!doctype html><html><body><p>{prose}</p>")
            } else {
                format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><text>{prose}</text>")
            };
            for child in children {
                body.push_str(&format!("<a href=\"/{}\">related</a>\n", self.name(child)));
            }
            body.push_str("<!-- <img src=\"example.svg\"> -->");
            body.push_str(if extension == "html" {
                "</body></html>"
            } else {
                "</svg>"
            });
            return body.into_bytes();
        }
        let header = if extension == "excalidraw.md" {
            "---\nexcalidraw-plugin: parsed\n---\n".into()
        } else if self.frontmatter_every > 0 && id % self.frontmatter_every == 0 {
            format!("---\ntag: group-{}\nsensitive: true\n---\n", id % 17)
        } else if self.frontmatter_every > 1 && id % self.frontmatter_every == 1 {
            "---\nunrelated: [malformed\n---\n".into()
        } else {
            String::new()
        };
        let mut body =
            format!("{header}# Note {id}\n\nA deterministic context graph benchmark. {prose}\n");
        for child in children {
            let target = self.name(child);
            match child % 3 {
                0 => body.push_str(&format!("[[{target}|related]]\n")),
                1 => body.push_str(&format!("[related](/{target})\n")),
                _ => body.push_str(&format!("<a href=\"/{target}\">related</a>\n")),
            }
        }
        // Ambiguous leaf links exercise resolution without making the ordinary
        // bounded query a whole-graph traversal.
        if id * self.branching + 1 >= self.files {
            body.push_str("[[shared]]\n");
        }
        body.push_str("`[[example]]`\n<!-- <img src=\"example.svg\"> -->\n");
        body.into_bytes()
    }
}
fn generate(directory: &Path, mut corpus: Corpus) -> Result<()> {
    anyhow::ensure!(
        corpus.files >= 100 && corpus.branching > 0 && corpus.directory_size > 1,
        "Use at least 100 files, positive branching, and directory-size > 1"
    );
    fs::create_dir(directory)
        .context("Use a new corpus directory; existing data is never overwritten")?;
    let root = directory.join("source");
    fs::create_dir(&root)?;
    let timer = Instant::now();
    let mut digest = Sha256::new();
    for id in 0..corpus.files {
        let name = corpus.name(id);
        let bytes = corpus.body(id);
        if id % corpus.directory_size == 0 {
            fs::create_dir(root.join(format!("{:04}", id / corpus.directory_size)))?;
        }
        fs::write(root.join(&name), &bytes)?;
        digest.update(name.as_bytes());
        digest.update(&bytes);
        corpus.source_bytes += bytes.len();
        if id > 0 && id % 50_000 == 0 {
            eprintln!("Generated {id}/{} files", corpus.files);
        }
    }
    corpus.sha256 = format!("{:x}", digest.finalize());
    fs::write(
        directory.join("corpus.json"),
        serde_json::to_vec_pretty(&corpus)?,
    )?;
    println!(
        "{}",
        json!({"event":"generated","corpus":corpus,"elapsedMs":timer.elapsed().as_secs_f64()*1000.0})
    );
    Ok(())
}
fn run_cli(
    cli: &Path,
    request: &Request,
    request_path: &Path,
) -> Result<(Response, f64, Option<u64>, usize)> {
    fs::write(request_path, serde_json::to_vec(request)?)?;
    let timed = Path::new("/usr/bin/time").exists() && cfg!(unix);
    let mut command = if timed {
        let mut command = Process::new("/usr/bin/time");
        if cfg!(target_os = "macos") {
            command.arg("-l");
        } else {
            command.args(["-f", "LINKRANGE_RSS_KIB=%M"]);
        }
        command.arg(cli);
        command
    } else {
        Process::new(cli)
    };
    command.args(["query", "--request"]).arg(request_path);
    let timer = Instant::now();
    let output = command.output()?;
    let elapsed = timer.elapsed().as_secs_f64() * 1000.0;
    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::ensure!(output.status.success(), "CLI failed: {stderr}");
    let rss = stderr.lines().find_map(|line| {
        if line.contains("maximum resident set size") {
            line.split_whitespace().next()?.parse::<u64>().ok()
        } else {
            line.strip_prefix("LINKRANGE_RSS_KIB=")?
                .parse::<u64>()
                .ok()
                .map(|n| n * 1024)
        }
    });
    let response: Response = serde_json::from_slice(&output.stdout)?;
    anyhow::ensure!(response.metrics.is_some(), "CLI omitted requested metrics");
    Ok((response, elapsed, rss, output.stdout.len()))
}
fn measure(directory: &Path, repeats: usize, selected: &str, cli: Option<PathBuf>) -> Result<()> {
    let corpus: Corpus = serde_json::from_slice(&fs::read(directory.join("corpus.json"))?)?;
    anyhow::ensure!(
        corpus.generator == "context-tree-v2",
        "Unknown corpus generator"
    );
    let cli = cli.unwrap_or(
        std::env::current_exe()?
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("linkrange"),
    );
    let root = directory.join("source");
    let request_path = directory.join("request.json");
    let request = Request {
        source_root: Some(root.clone()),
        sources: None,
        index: IndexOptions {
            cache_directory: Some(directory.join("cache")),
            frontmatter: vec![FrontmatterField {
                key: "sensitive".into(),
                substring: None,
            }],
            ..Default::default()
        },
        query: Query {
            starts: vec![Start {
                path: corpus.name(0),
                depths: None,
            }],
            depths: Depths {
                outlinks: 3,
                inlinks: 0,
            },
            adjacency: true,
            metrics: true,
            ..Default::default()
        },
    };
    let scenarios = [
        "cold",
        "warm",
        "incremental",
        "add",
        "delete",
        "rename",
        "metadata-change",
        "no-cache",
        "wide",
    ];
    anyhow::ensure!(
        repeats > 0 && (selected == "all" || scenarios.contains(&selected)),
        "Invalid repeats or scenario"
    );
    run_cli(&cli, &request, &request_path)?;
    let command_text = |program: &str, args: &[&str]| {
        Process::new(program)
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    let revision = command_text("git", &["rev-parse", "HEAD"]);
    let toolchain = command_text("rustc", &["--version"]);
    for scenario in scenarios
        .into_iter()
        .filter(|case| selected == "all" || *case == selected)
    {
        for sample in 0..repeats {
            let mut query = request.clone();
            query.index.rebuild = scenario == "cold";
            query.index.no_cache = scenario == "no-cache";
            if scenario == "wide" {
                query.query.depths.outlinks = 8;
            }
            if scenario == "metadata-change" {
                query.index.frontmatter.push(FrontmatterField {
                    key: "tag".into(),
                    substring: None,
                });
            }
            let original_path = root.join(corpus.name(corpus.files - 1));
            let original = fs::read(&original_path)?;
            let added = root.join("benchmark-added.md");
            let renamed = original_path.with_file_name("benchmark-renamed.md");
            match scenario {
                "incremental" => {
                    let mut content = original.clone();
                    content.extend_from_slice(b"\ncontent-only edit\n");
                    fs::write(&original_path, content)?;
                }
                "add" => fs::write(&added, format!("[[{}]]", corpus.name(0)))?,
                "delete" => fs::remove_file(&original_path)?,
                "rename" => fs::rename(&original_path, &renamed)?,
                _ => {}
            }
            // Always restore controlled edits, even when the candidate fails.
            let measured = run_cli(&cli, &query, &request_path);
            match scenario {
                "incremental" | "delete" => fs::write(&original_path, &original)?,
                "add" => fs::remove_file(&added)?,
                "rename" => fs::rename(&renamed, &original_path)?,
                _ => {}
            }
            let (response, total, rss, bytes) = measured?;
            let expected = (0..=query.query.depths.outlinks)
                .map(|n| corpus.branching.pow(n))
                .sum::<usize>()
                .min(corpus.files);
            anyhow::ensure!(
                response.complete && response.nodes.len() == expected,
                "Traversal correctness failure: expected {expected}, got {}",
                response.nodes.len()
            );
            let mut semantic = serde_json::to_value(&response)?;
            semantic.as_object_mut().unwrap().remove("metrics");
            let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&semantic)?));
            println!(
                "{}",
                json!({"schemaVersion":2,"event":"measurement","scenario":scenario,"sample":sample,"revision":revision,"toolchain":toolchain,
                "platform":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"threads":std::thread::available_parallelism().map(|n|n.get()).ok()},
                "corpus":corpus,"totalMs":total,"metrics":response.metrics,"peakProcessRssBytes":rss,"responseBytes":bytes,"nodes":response.nodes.len(),"edges":response.edges.len(),"correct":true,"semanticSha256":hash})
            );
            if matches!(
                scenario,
                "incremental" | "add" | "delete" | "rename" | "metadata-change"
            ) {
                run_cli(&cli, &request, &request_path)?;
            }
        }
    }
    Ok(())
}
fn main() -> Result<()> {
    match Args::parse().command {
        Command::Generate {
            directory,
            files,
            branching,
            directory_size,
            frontmatter_every,
            prose_words,
            seed,
        } => generate(
            &directory,
            Corpus {
                schema_version: 2,
                generator: "context-tree-v2".into(),
                files,
                branching,
                directory_size,
                frontmatter_every,
                prose_words,
                seed,
                source_bytes: 0,
                sha256: String::new(),
            },
        ),
        Command::Measure {
            directory,
            repeats,
            scenario,
            cli,
        } => measure(&directory, repeats, &scenario, cli),
    }
}
