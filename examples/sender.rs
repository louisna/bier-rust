#[macro_use]
extern crate log;

use bier_rust::api::SendInfo;
use clap::Parser;
use socket2;

#[derive(Parser)]
struct Args {
    /// Path to the BIER daemon.
    #[clap(short = 'b', long = "bier", value_parser)]
    bier_path: String,
    /// Number of packets to send.
    #[clap(short = 'n', value_parser, default_value = "1")]
    nb_to_send: usize,
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    // Sock used to send messages.
    // No need to bind the socket as we only send messages.
    let sock = socket2::Socket::new(
        socket2::Domain::UNIX,
        socket2::Type::DGRAM,
        None
    ).unwrap();
    
}