#[macro_use]
extern crate log;

use std::os::unix::prelude::AsRawFd;

use clap::Parser;

use bier_rust::api::CommunicationInfo;
use bier_rust::bier::BierState;
use serde_json::{from_reader, from_value, Value};

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

const TOKEN_IP_SOCK: mio::Token = mio::Token(0);
const TOKEN_UNIX_SOCK: mio::Token = mio::Token(1);

fn main() {
    env_logger::init();
    let args = Args::parse();

    let file = std::fs::File::open(args.config).expect("Cannot find the file");
    let json: Value = from_reader(file).expect("Cannot read the JSON content");
    let bier_state: BierState = from_value(json).expect("Cannot parse the JSON to BierState");

    let _ = std::fs::remove_file(&args.bier_unix_path);
    let bier_unix_sock =
        socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::DGRAM, None).unwrap();
    bier_unix_sock
        .bind(&socket2::SockAddr::unix(&args.bier_unix_path).unwrap())
        .unwrap();

    let bier_ip_sock = socket2::Socket::new(
        socket2::Domain::IPV6,
        socket2::Type::RAW,
        Some(socket2::Protocol::from(253)),
    )
    .expect("Impossible to create the IP raw socket with proto");

    let mut poll = mio::Poll::new().unwrap();
    let mut events = mio::Events::with_capacity(1024);

    // Register the sockets.
    poll.registry()
        .register(
            &mut mio::unix::SourceFd(&bier_ip_sock.as_raw_fd()),
            TOKEN_IP_SOCK,
            mio::Interest::READABLE,
        )
        .unwrap();
    poll.registry()
        .register(
            &mut mio::unix::SourceFd(&bier_unix_sock.as_raw_fd()),
            TOKEN_UNIX_SOCK,
            mio::Interest::READABLE,
        )
        .unwrap();

    let mut buffer: Vec<u8> = Vec::with_capacity(4096);
    let mut output_buff = vec![0u8; 2048];

    // Start listening for BIER packets.
    // TOKEN_IP_SOCK: receives a BIER packet from the network.
    // TOKEN_UNIX_SOCK: receives a packet from an application to send in the network.
    loop {
        poll.poll(&mut events, None).unwrap();

        if events.is_empty() {
            debug!("Events is empty");
            break;
        }

        for event in &events {
            unsafe {
                buffer.set_len(0);
            }

            let (bier_header, packet) = if event.token() == TOKEN_UNIX_SOCK {
                // Received a multicast payload locally by an upper-layer program.
                let (read, _) = bier_unix_sock
                    .recv_from(buffer.spare_capacity_mut())
                    .unwrap();
                
                    unsafe {
                        buffer.set_len(read);
                    }

                // Parse the payload of the user to get the BIER information as well as the payload.
                debug!("Received buffer of length: {:?} with last byte: {}", read, &buffer[read - 1]);
                let recv_info = CommunicationInfo::from_slice(&buffer[..read]).unwrap();

                let bier_header = match bier_rust::header::BierHeader::from_recv_info(&recv_info) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Impossible to get a BIER header from UNIX: {:?}", e);
                        continue;
                    }
                };
                bier_header.to_slice(&mut output_buff[..]).unwrap();

                // Copy the payload.
                output_buff[bier_header.header_length()..bier_header.header_length() + recv_info.payload.len()].copy_from_slice(recv_info.payload);

                let packet =
                    &mut output_buff[..bier_header.header_length() + recv_info.payload.len()];
                (bier_header, packet)
            } else if event.token() == TOKEN_IP_SOCK {
                debug!("Received a packet from IP");
                // Received a BIER packet from the network.
                let (read, _) = bier_ip_sock.recv_from(buffer.spare_capacity_mut()).unwrap();
                unsafe {
                    buffer.set_len(read);
                }
                
                let bier_header = bier_rust::header::BierHeader::from_slice(&buffer[..read])
                    .expect("Cannot convert the BIER header");

                (bier_header, &mut buffer[..read])
            } else {
                error!("Unrecognized token: {:?}", event.token());
                continue;
            };
            let bier_next_hops = match bier_state
                .process_bier(&bier_header.get_bitstring(), bier_header.get_bift_id())
            {
                Ok(v) => v,
                Err(e) => {
                    debug!(
                        "Error when processing the BIER packet: {:?}, continuing...",
                        e
                    );
                    continue;
                }
            };

            // For each next-hop, send the modified packet to the socket with the IP tunnel.
            for (bitstring, nxt_hop) in bier_next_hops {
                // Update the BIER bitstring with the provided bitstring.
                match bitstring.update_header_from_self(packet) {
                    Ok(_) => debug!("Updated the header"),
                    Err(e) => {
                        debug!("Error when updating the packet: {:?}, continuing...", e);
                        continue;
                    }
                }

                if let Some(dst) = nxt_hop {
                    // Send it to the IP socket.
                    let sock_addr = std::net::SocketAddr::new(dst, 0);
                    match bier_ip_sock.send_to(packet, &sock_addr.into()) {
                        Ok(_) => debug!("Sent the packet to {:?}", dst),
                        Err(e) => {
                            debug!("Error when sending the packet to {:?}. Error is: {:?}, continuing...", dst, e);
                            continue;
                        }
                    }
                } else {
                    // This BFER is the destination of the packet. Send it locally to the upper-layer.
                    // For the upper-layer program, we remove the BIER header.
                    let payload = &packet[bier_header.header_length()..];
                    if let Some(def_app_path) = &args.default_unix_path {
                        let dst = socket2::SockAddr::unix(def_app_path).unwrap();
                        match bier_unix_sock.send_to(payload, &dst) {
                            Ok(_) => debug!(
                                "Sent a packet to the local default program: {}",
                                def_app_path
                            ),
                            Err(e) => {
                                debug!("Error when sending a packet to the local default program: {}. Error is: {:?}, continuing...", def_app_path, e);
                                continue;
                            }
                        }
                    }
                }
            }
        }
    }
}
