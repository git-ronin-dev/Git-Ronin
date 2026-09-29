//! Times graph loading on a real repository:
//! `cargo run --release -p ronin-git --example graph_bench -- <repo>`

use std::path::PathBuf;
use std::time::Instant;

use ronin_git::{GitCli, Graph, GraphFilter};

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).expect("usage: graph_bench <repo>"));
    let git = GitCli::discover().unwrap();

    let start = Instant::now();
    let mut graph = Graph::open(&git, &path, &GraphFilter::default()).unwrap();
    println!("open:            {:?}", start.elapsed());

    let t = Instant::now();
    graph.page(0, 200).unwrap();
    println!("first 200 rows:  {:?}", t.elapsed());

    let t = Instant::now();
    let page = graph.page(0, usize::MAX).unwrap();
    println!(
        "all {} rows:  {:?} (max {} lanes)",
        page.loaded,
        t.elapsed(),
        page.max_lanes
    );

    let t = Instant::now();
    let hits = graph.search("refs").unwrap();
    println!("search 'refs':   {:?} ({} hits)", t.elapsed(), hits.len());
}
