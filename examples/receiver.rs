#[macro_use]
extern crate log;

use clap::Parser;
use socket2;
use bier_rust::api::RecvInfo;

#[derive(Parser)]
struct Args {
    /// Path to the BIER daemon.
    #[clap(short = 'u', long = "unix-path", value_parser)]
    unix_path: String,
    /// Number of packets to listen.
    #[clap(short = 'n', value_parser, default_value = "1")]
    nb_to_recv: usize,
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
    let recv_addr = socket2::SockAddr::unix(args.unix_path).unwrap();
    sock.bind(&recv_addr).unwrap();

    let mut buffer = Vec::with_capacity(4096);
    for _ in 0..args.nb_to_recv {
        let read = sock.recv(buffer.spare_capacity_mut()).unwrap();
        let recv_info = RecvInfo::from_slice(&buffer[..read]).unwrap();
        debug!("Received {} bytes", recv_info.payload.len());
    }
}