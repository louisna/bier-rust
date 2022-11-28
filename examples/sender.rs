#[macro_use]
extern crate log;

use std::str::FromStr;

use bier_rust::api::SendInfo;
use bier_rust::bier::Bitstring;
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
    /// Destination multicast address.
    #[clap(short = 'a', long = "multicast-address", value_parser)]
    mc_dst: String,
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

    // Put data in the packet buffer.
    let mut buffer = [0u8; 4096];
    let packet = [0u8; 1000];
    let bitstring = Bitstring::from_str("11001").unwrap();
    let bitstring: Vec<u8> = (&bitstring).into();

    // Create the send info and the slice from it.
    let send_info = SendInfo {
        bift_id: 1,
        bitstring: &bitstring,
        payload: &packet,
    };

    let bier_addr = socket2::SockAddr::unix(args.bier_path).unwrap();
    let size = send_info.to_slice(&mut buffer[..]).unwrap();
    for _ in 0..args.nb_to_send {
        sock.send_to(&buffer[..size], &bier_addr).unwrap();
        debug!("Sent a message to BIER process");
    }
}