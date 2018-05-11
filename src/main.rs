use std::fs::File;
use std::io::prelude::*;

struct GossipEvent {
    hash: String,
}

struct Node {
    name: String,
    gossip: Vec<GossipEvent>,
}

fn read(file: std::io::Result<File>) -> std::io::Result<bool> {
    let mut contents = String::new();
    file?.read_to_string(&mut contents)?;
    let nodes = contents
        .split("subgraph")
        .map(|s| s.trim())
        .filter(|s| s.starts_with("cluster_"))
        .map(|s| {
            let content = s.split('{').nth(1).unwrap().split('}').nth(0).unwrap();
            let name = s.split("cluster_")
                .last()
                .unwrap()
                .split(' ')
                .nth(0)
                .unwrap();
            println!("Node: {}\nContent:\n{}\n", name, content);
            (name, content)
        })
        .map(|(name, content)| {
            let events = content;
            (name, events)
        })
        .collect::<Vec<_>>();
    println!("{:?}", nodes);
    Ok(true)
}

fn write(file: std::io::Result<File>, gossip_graph: &bool) -> std::io::Result<bool> {
    file?;

    Ok(*gossip_graph)
}

fn main() {
    // TODO: take input file from CLI args with clap
    // Also take type of annotation to produce from there
    let input_filename = "input.dot";
    let gossip_graph = read(File::open(input_filename)).unwrap();
    write(File::create("output.dot"), &gossip_graph).unwrap();
}
