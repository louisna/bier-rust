use bier_rust::bier::{BierEntryPath, BierState, Bift, BiftEntry, Bitstring};
use bier_rust::dijkstra::dijkstra;
use clap::Parser;
use serde_json::to_writer;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::str::FromStr;

#[derive(Debug)]
enum Error {
    /// Impossible to parse the file to crate a topo.
    FileParse,
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser)]
struct Args {
    /// Topology NTF-like file.
    #[clap(short = 'f', long = "topo-file", value_parser)]
    topo_file: String,
    /// Path containing the output files.
    #[clap(short = 'd', long = "directory", value_parser)]
    directory: String,
    /// Mapping between node and IPv6 address.
    #[clap(short = 'i', long = "node2ipv6", value_parser)]
    node_to_ipv6: String,
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    let graph = Graph::from_file(&args.topo_file, &args.node_to_ipv6).unwrap();
    let path = std::path::Path::new(&args.topo_file);
    let filename = path.file_stem().unwrap().to_str().unwrap();
    graph.get_bier_config(&args.directory, filename).unwrap();
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Node {
    id: usize, // Used as bitstring ID.
    name: String,
    neighbours: Vec<(usize, i32)>, // (id, cost)
    loopback: IpAddr,
}

struct Graph {
    nodes: Vec<Node>,
}

impl Graph {
    fn from_file(file_path: &str, node_to_ipv6_file: &str) -> Result<Self> {
        let file = std::fs::File::open(file_path).map_err(|_| Error::FileParse)?;
        let node_to_ipv6 = std::fs::File::open(node_to_ipv6_file).map_err(|_| Error::FileParse)?;

        // Form a vector of the mapping, assuming that we have 0 first, then 1, then ...
        let reader = BufReader::new(node_to_ipv6);
        let node_to_ipv6: Vec<_> = reader
            .lines()
            .map(|line| {
                let line = line.unwrap();
                let line = line.trim().trim_end();
                println!("What is this line: {}", line);
                let ip_str = line.split(' ').nth(1)?.split('/').next()?;
                println!("IP STR: {}", ip_str);
                Some(IpAddr::V6(ip_str.parse().ok()?))
            })
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .unwrap();

        let mut nodes = Vec::new(); // We do not know the size at first.
        let reader = BufReader::new(file);
        let mut node2id = HashMap::new();
        let mut current_id = 0;

        for line in reader.lines() {
            let line = line.unwrap();
            let line = line.trim().trim_end();
            if line.is_empty() {
                continue;
            }
            let split: Vec<&str> = line.split(' ').collect();
            let a_id: usize = *node2id.entry(split[0].to_string()).or_insert(current_id);
            if a_id == current_id {
                current_id += 1;
                let node = Node {
                    name: split[0].to_string(),
                    neighbours: Vec::new(),
                    id: a_id,
                    loopback: node_to_ipv6[a_id],
                };
                nodes.push(node);
            }

            let b_id: usize = *node2id.entry(split[1].to_string()).or_insert(current_id);
            if b_id == current_id {
                current_id += 1;
                let node = Node {
                    name: split[1].to_string(),
                    neighbours: Vec::new(),
                    id: b_id,
                    loopback: node_to_ipv6[b_id],
                };
                nodes.push(node);
            }

            // Get the metric from the line
            let metric: i32 = split[2].parse::<i32>().unwrap();

            // Add in neighbours adjacency list
            nodes[a_id].neighbours.push((b_id, metric));
            nodes[b_id].neighbours.push((a_id, metric));
        }

        Ok(Graph { nodes })
    }

    fn graph_node_to_usize(&self) -> Vec<Vec<(usize, i32)>> {
        self.nodes
            .iter()
            .map(|node| node.neighbours.to_owned())
            .collect()
    }

    fn get_bier_config(&self, directory: &str, filename_root: &str) -> Result<()> {
        let nodes = &self.nodes;
        let nb_nodes = nodes.len();
        let graph_id = self.graph_node_to_usize();

        for node in 0..nb_nodes {
            // Predecessor(s) for each node, alongside the shortest path(s) from `node`
            let predecessors = dijkstra(&graph_id, &node).unwrap();

            // Construct the next hop mapping, possibly there are multiple paths so multiple output interfaces
            let next_hop: Vec<Vec<usize>> = (0..nb_nodes)
                .map(|i| get_all_out_interfaces_to_destination(&predecessors, node, i))
                .collect();

            let mut bift = Bift {
                bift_id: 1,
                bift_type: bier_rust::bier::BiftType::Bier,
                bfr_id: node as u64 + 1,
                entries: Vec::new(),
            };

            for bfr_id in 0..nb_nodes {
                let mut entry = BiftEntry {
                    bit: bfr_id as u64,
                    paths: Vec::new(),
                };
                for &the_next_hop in &next_hop[bfr_id] {
                    let s = next_hop.iter().rev().fold(String::new(), |mut fbm, nh| {
                        if nh.contains(&the_next_hop) {
                            fbm.push('1');
                            fbm
                        } else {
                            if !fbm.is_empty() {
                                fbm.push('0');
                            }
                            fbm
                        }
                    });
                    let bitstring: Bitstring = FromStr::from_str(&s).unwrap();
                    entry.paths.push(BierEntryPath {
                        bitstring,
                        next_hop: nodes[the_next_hop].loopback,
                    });
                }
                bift.entries.push(entry);
            }

            let bier_state = BierState {
                loopback: nodes[node].loopback,
                bifts: vec![bift],
            };

            let pathname = format!("{}-{}.json", filename_root, node);
            let path = std::path::Path::new(directory).join(&pathname);
            let file = std::fs::File::create(&path).unwrap();
            to_writer(file, &bier_state).unwrap();
        }

        Ok(())
    }
}

fn get_all_out_interfaces_to_destination(
    predecessors: &HashMap<&usize, Vec<&usize>>,
    source: usize,
    destination: usize,
) -> Vec<usize> {
    if source == destination {
        return vec![source];
    }

    let mut out: Vec<usize> = Vec::new();
    let mut visited = vec![false; predecessors.len()];
    let mut stack = VecDeque::new();
    stack.push_back(destination);
    while !stack.is_empty() {
        let elem = stack.pop_back().unwrap();
        if visited[elem] {
            continue;
        }
        visited[elem] = true;
        for &&pred in predecessors.get(&elem).unwrap() {
            if pred == source {
                out.push(elem);
                continue;
            }
            if visited[pred] {
                continue;
            }
            stack.push_back(pred);
        }
    }
    out
}

#[cfg(test)]
mod tests {

    use bier_rust::bier::BierState;

    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    const TEST_DIRECTORY: &str = "test_configs";
    const EXPECTED_CONFIGURATIONS: [&str; 5] = [
        r#"{"loopback":"babe:cafe::1","bifts":[{"bift_id":1,"bift_type":1,"bfr_id":1,"entries":[{"bit":0,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000000001","next_hop":"babe:cafe::1"}]},{"bit":1,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000011010","next_hop":"babe:cafe:1::1"}]},{"bit":2,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000011100","next_hop":"babe:cafe:2::1"}]},{"bit":3,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000011010","next_hop":"babe:cafe:1::1"},{"bitstring":"0000000000000000000000000000000000000000000000000000000000011100","next_hop":"babe:cafe:2::1"}]},{"bit":4,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000011010","next_hop":"babe:cafe:1::1"},{"bitstring":"0000000000000000000000000000000000000000000000000000000000011100","next_hop":"babe:cafe:2::1"}]}]}]}"#,
        r#"{"loopback":"babe:cafe:1::1","bifts":[{"bift_id":1,"bift_type":1,"bfr_id":2,"entries":[{"bit":0,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000000101","next_hop":"babe:cafe::1"}]},{"bit":1,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000000010","next_hop":"babe:cafe:1::1"}]},{"bit":2,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000000101","next_hop":"babe:cafe::1"},{"bitstring":"0000000000000000000000000000000000000000000000000000000000011100","next_hop":"babe:cafe:3::1"}]},{"bit":3,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000011100","next_hop":"babe:cafe:3::1"}]},{"bit":4,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000011100","next_hop":"babe:cafe:3::1"}]}]}]}"#,
        r#"{"loopback":"babe:cafe:2::1","bifts":[{"bift_id":1,"bift_type":1,"bfr_id":3,"entries":[{"bit":0,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000000011","next_hop":"babe:cafe::1"}]},{"bit":1,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000000011","next_hop":"babe:cafe::1"},{"bitstring":"0000000000000000000000000000000000000000000000000000000000011010","next_hop":"babe:cafe:3::1"}]},{"bit":2,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000000100","next_hop":"babe:cafe:2::1"}]},{"bit":3,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000011010","next_hop":"babe:cafe:3::1"}]},{"bit":4,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000011010","next_hop":"babe:cafe:3::1"}]}]}]}"#,
        r#"{"loopback":"babe:cafe:3::1","bifts":[{"bift_id":1,"bift_type":1,"bfr_id":4,"entries":[{"bit":0,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000000011","next_hop":"babe:cafe:1::1"},{"bitstring":"0000000000000000000000000000000000000000000000000000000000000101","next_hop":"babe:cafe:2::1"}]},{"bit":1,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000000011","next_hop":"babe:cafe:1::1"}]},{"bit":2,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000000101","next_hop":"babe:cafe:2::1"}]},{"bit":3,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000001000","next_hop":"babe:cafe:3::1"}]},{"bit":4,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000010000","next_hop":"babe:cafe:4::1"}]}]}]}"#,
        r#"{"loopback":"babe:cafe:4::1","bifts":[{"bift_id":1,"bift_type":1,"bfr_id":5,"entries":[{"bit":0,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000001111","next_hop":"babe:cafe:3::1"}]},{"bit":1,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000001111","next_hop":"babe:cafe:3::1"}]},{"bit":2,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000001111","next_hop":"babe:cafe:3::1"}]},{"bit":3,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000001111","next_hop":"babe:cafe:3::1"}]},{"bit":4,"paths":[{"bitstring":"0000000000000000000000000000000000000000000000000000000000010000","next_hop":"babe:cafe:4::1"}]}]}]}"#,
    ];

    /// This is an "extended" diamond topology.
    ///     a
    ///   /   \
    ///  b     c
    ///   \   /
    ///     d
    ///     |
    ///     e
    fn write_dummy_topo(path: &Path) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        let content = r#"a b 1 1
        a c 1 1
        b d 1 1
        c d 1 1
        d e 1 1
        "#;
        write!(file, "{}", content)
    }

    fn write_dummy_node_to_ipv6(path: &Path) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        let content = r#"0 babe:cafe:0::1/64
        1 babe:cafe:1::1/64
        2 babe:cafe:2::1/64
        3 babe:cafe:3::1/64
        4 babe:cafe:4::1/64
"#;

        write!(file, "{}", content)
    }

    fn get_bier_state_from_path(path: &Path) -> Result<BierState> {
        let content = std::fs::read_to_string(path).map_err(|_| Error::FileParse)?;
        serde_json::from_str(&content).map_err(|_| Error::FileParse)
    }

    #[test]
    /// Tests the BIER configuration build.
    fn test_bier_configuration() {
        // Test setup.
        let dir_path = Path::new(TEST_DIRECTORY);
        if dir_path.exists() {
            std::fs::remove_dir_all(dir_path).unwrap();
        }
        std::fs::create_dir(dir_path).unwrap();

        let topo_path = dir_path.join("topo.ntf");
        write_dummy_topo(&topo_path).unwrap();

        let node_to_ipv6_path = dir_path.join("node_to_ipv6.ntf");
        write_dummy_node_to_ipv6(&node_to_ipv6_path).unwrap();

        // Actual test.
        let graph = Graph::from_file(
            topo_path.to_str().unwrap(),
            node_to_ipv6_path.to_str().unwrap(),
        );
        assert!(graph.is_ok());
        let graph = graph.unwrap();
        let res = graph.get_bier_config(
            TEST_DIRECTORY,
            topo_path.file_stem().unwrap().to_str().unwrap(),
        );
        assert!(res.is_ok());

        // The parsing worked. Now we have to check the BIFTs if the paths are correctly encoded.
        for node_id in 0..5 {
            let bier_state =
                get_bier_state_from_path(&dir_path.join(format!("topo-{}.json", node_id)));
            assert!(bier_state.is_ok());
            let bier_state = bier_state.unwrap();
            let expected: BierState =
                serde_json::from_str(EXPECTED_CONFIGURATIONS[node_id]).unwrap();
            assert_eq!(bier_state, expected);
        }

        // Clean test.
        std::fs::remove_dir_all(dir_path).unwrap();
    }
}
