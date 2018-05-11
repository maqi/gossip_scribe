
fn read(file: std::io::Result<std::fs::File>) -> std::io::Result<bool> {
    file?;
    Ok(true)
}

fn write(file: std::io::Result<std::fs::File>, gossip_graph: &bool) -> std::io::Result<bool> {
    file?;
    Ok(*gossip_graph)
}

fn main() {
    // TODO: take input file from CLI args with clap
    let input_filename = "input.dot";
    let gossip_graph = read(std::fs::File::open(input_filename)).unwrap();
    write(std::fs::File::open("output.dot"), &gossip_graph).unwrap();
    println!("Hello, world!");
}
