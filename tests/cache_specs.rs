use linkrange::{Graph, IndexOptions, Query, Source, Start};
use std::{fs, path::Path};

fn sources(root: &Path) -> Vec<Source> {
    vec![Source {
        name: "source".into(),
        directory: root.into(),
        aliases: Vec::new(),
    }]
}

fn fixture() -> (tempfile::TempDir, IndexOptions) {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("vault")).unwrap();
    let options = IndexOptions {
        cache_directory: Some(temp.path().join("cache")),
        ..Default::default()
    };
    (temp, options)
}
fn cache_file(dir: &Path) -> std::path::PathBuf {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .unwrap()
}
#[test]
fn incompatible_and_interrupted_caches_are_rebuilt_and_abandoned_staging_is_ignored() {
    let (temp, options) = fixture();
    let root = temp.path().join("vault");
    fs::write(root.join("A.md"), "[[B]]").unwrap();
    Graph::open(&sources(&root), &options).unwrap();
    let path = cache_file(options.cache_directory.as_ref().unwrap());
    let mut cache: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    cache["version"] = "incompatible".into();
    fs::write(&path, serde_json::to_vec(&cache).unwrap()).unwrap();
    assert!(
        Graph::open(&sources(&root), &options)
            .unwrap()
            .metrics()
            .cache_rebuilt
    );
    fs::write(&path, b"interrupted garbage").unwrap();
    assert_eq!(
        Graph::open(&sources(&root), &options)
            .unwrap()
            .metrics()
            .files_read,
        1
    );
    fs::write(path.with_extension("abandoned.tmp"), b"partial").unwrap();
    let graph = Graph::open(&sources(&root), &options).unwrap();
    assert_eq!(graph.files().count(), 1);
    #[cfg(unix)]
    assert_eq!(graph.metrics().files_read, 0);
}
#[test]
fn simultaneous_rebuilds_each_return_a_complete_index_and_publish_a_reusable_cache() {
    let (temp, options) = fixture();
    let root = temp.path().join("vault");
    for id in 0..20 {
        fs::write(root.join(format!("{id}.md")), "[[0]]").unwrap();
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    Graph::open(
                        &sources(&root),
                        &IndexOptions {
                            rebuild: true,
                            ..options.clone()
                        },
                    )
                    .unwrap()
                })
            })
            .collect();
        for handle in handles {
            let graph = handle.join().unwrap();
            assert_eq!(graph.files().count(), 20);
            assert_eq!(graph.metrics().files_read, 20);
        }
    });
    let graph = Graph::open(&sources(&root), &options).unwrap();
    assert_eq!(graph.files().count(), 20);
    #[cfg(unix)]
    assert_eq!(graph.metrics().files_read, 0);
}
#[cfg(unix)]
#[test]
fn changed_time_detects_same_size_edits_even_when_modification_time_is_restored() {
    use std::io::Write;
    let (temp, options) = fixture();
    let root = temp.path().join("vault");
    let path = root.join("A.md");
    fs::write(&path, "[[Alpha]]").unwrap();
    let first = Graph::open(&sources(&root), &options).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.write_all(b"[[Bravo]]").unwrap();
    file.set_modified(modified).unwrap();
    let next = Graph::open(&sources(&root), &options).unwrap();
    assert_eq!(next.metrics().files_read, 1);
    assert_ne!(
        first.files().next().unwrap().digest,
        next.files().next().unwrap().digest
    );
    let response = next
        .query(&Query {
            starts: vec![Start {
                path: "source://source/A.md".into(),
                depths: None,
            }],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        response.links_by_source["source://source/A.md"][0].link_original_text,
        "Bravo"
    );
}
#[test]
fn replacement_files_and_added_empty_directories_refresh_the_inventory() {
    let (temp, options) = fixture();
    let root = temp.path().join("vault");
    fs::write(root.join("A.md"), "[[Alpha]]").unwrap();
    let old = Graph::open(&sources(&root), &options).unwrap();
    fs::write(root.join("replacement.md"), "[[Bravo]]").unwrap();
    fs::rename(root.join("replacement.md"), root.join("A.md")).unwrap();
    fs::create_dir(root.join("empty")).unwrap();
    let next = Graph::open(&sources(&root), &options).unwrap();
    assert_eq!(next.metrics().files_read, 1);
    assert_ne!(
        old.files().next().unwrap().digest,
        next.files().next().unwrap().digest
    );
    assert!(next.directories().contains("source://source/empty"));
}
