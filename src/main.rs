use clap::{Parser, Subcommand};
use linkrange::{Depths, FrontmatterField, IndexOptions, Query, Request, Start, Symlinks};
use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(version, about = "Build bounded context graphs from linked files")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Query {
        /// Versioned query request JSON file; '-' reads standard input.
        #[arg(long, conflicts_with = "source_root")]
        request: Option<PathBuf>,
        #[arg(long)]
        source_root: Option<PathBuf>,
        #[arg(long)]
        start: Vec<String>,
        #[arg(long, default_value_t = 1)]
        outlinks: u32,
        #[arg(long, default_value_t = 0)]
        inlinks: u32,
        #[arg(long, default_value_t = 0)]
        frontier_depth: u32,
        #[arg(long)]
        cache_directory: Option<PathBuf>,
        #[arg(long)]
        no_cache: bool,
        #[arg(long)]
        rebuild: bool,
        #[arg(long)]
        best_effort: bool,
        #[arg(long)]
        follow_internal_symlinks: bool,
        #[arg(long)]
        frontmatter: Vec<String>,
        #[arg(long)]
        boundary_embed_type: Vec<String>,
        /// Include direct embeds from these source formats at the depth boundary.
        #[arg(long)]
        boundary_embed_source_type: Vec<String>,
        #[arg(long)]
        adjacency: bool,
        #[arg(long)]
        explain_resolution: bool,
        /// Include timing, parsing, and cache measurements in the response.
        #[arg(long)]
        metrics: bool,
    },
}

fn run() -> anyhow::Result<()> {
    let Cli {
        command:
            Command::Query {
                request,
                source_root,
                start,
                outlinks,
                inlinks,
                frontier_depth,
                cache_directory,
                no_cache,
                rebuild,
                best_effort,
                follow_internal_symlinks,
                frontmatter,
                boundary_embed_type,
                boundary_embed_source_type,
                adjacency,
                explain_resolution,
                metrics,
            },
    } = Cli::parse();
    let mut request = if let Some(path) = request {
        let mut text = String::new();
        if path.as_os_str() == "-" {
            io::stdin().read_to_string(&mut text)?;
        } else {
            text = fs::read_to_string(path)?;
        }
        serde_json::from_str::<Request>(&text)?
    } else {
        Request {
            source_root: Some(
                source_root
                    .ok_or_else(|| anyhow::anyhow!("--source-root or --request is required"))?,
            ),
            sources: None,
            index: IndexOptions {
                cache_directory,
                no_cache,
                rebuild,
                best_effort,
                symlinks: if follow_internal_symlinks {
                    Symlinks::FollowInternal
                } else {
                    Symlinks::Skip
                },
                frontmatter: frontmatter
                    .into_iter()
                    .map(|key| FrontmatterField {
                        key,
                        substring: None,
                    })
                    .collect(),
                ..Default::default()
            },
            query: Query {
                starts: start
                    .into_iter()
                    .map(|path| Start { path, depths: None })
                    .collect(),
                depths: Depths { outlinks, inlinks },
                frontier_depth,
                boundary_embed_types: boundary_embed_type,
                boundary_embed_source_types: boundary_embed_source_type,
                adjacency,
                explain_resolution,
                ..Default::default()
            },
        }
    };
    request.query.metrics |= metrics;
    let result = linkrange::query(&request)?;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer(&mut stdout, &result)?;
    stdout.write_all(b"\n")?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!(
            "{}",
            serde_json::json!({"schemaVersion":1,"code":"queryFailed","message":format!("{error:#}")})
        );
        std::process::exit(1);
    }
}
