extern crate log;

use clap::Parser;

use bier_rust::bier::BierState;
use serde_json::{from_reader, from_value, Value};
use socket2;

#[derive(Parser)]
struct Args {
    /// Path to the configuration file of the BFR.
    #[clap(
        short = 'c',
        long = "config",
        value_parser,
        default_value = "configs/example.json"
    )]
    config: String,
    /// Default UNIX socket address to forward the packets received by this BFER.
    /// None by default.
    #[clap(short = 'd', long = "default", value_parser)]
    default_unix_path: Option<String>,
    /// UNIX socket address of the BIER daemon.
    #[clap(long = "bier-path", value_parser)]
    bier_unix_path: String,
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    let file = std::fs::File::open(args.config).expect("Cannot find the file");
    let json: Value = from_reader(file).expect("Cannot read the JSON content");
    let bier_state: BierState = from_value(json).expect("Cannot parse the JSON to BierState");

    let default_sock = &args.default_unix_path.map(|path| {
        std::fs::remove_file(&path).unwrap();
        let sock = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::DGRAM, None)
            .unwrap_or_else(|_| panic!("Impossible to open a UNIX socket for this path: {}", path));
        sock.bind(socket2::SockAddr::unix(path)).unwrap_or_else(|e| panic!("Impossible to bind the default socket for this path: {} - {:?}", path, e))
    });

    std::fs::remove_file(&args.bier_unix_path).unwrap();
    let bier_unix_sock = UnixDatagram::bind(&args.bier_unix_path).unwrap_or_else(|e| {
        panic!(
            "Impossible to open a UNIX socket for the BIER daemon for this path: {} (error: {:?})",
            &args.bier_unix_path, e
        )
    });

    let bier_sock = socket2::Socket::new(socket2::Domain::IPV6, socket2::Type::RAW, Some(socket2::Protocol::from(253))).expect("Impossible to create the IP raw socket with proto");


}