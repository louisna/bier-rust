#[macro_use]
extern crate log;

use clap::Parser;

use serde_json::{Value, from_reader, from_value};
use bier_rust::bift::BierState;

#[derive(Parser)]
struct Args {
    #[clap(short = 'c', long = "config")]
    config: String,
}

fn main() {
    env_logger::init();
    let args = Args::parse();
    println!("Hello, world!");

    let file = std::fs::File::open("configs/example.json").expect("Cannot find the file");
    let json: Value = from_reader(file).expect("Cannot read the JSON content");
    debug!("JSON value: {:?}", json);
    let a: bier_rust::bift::Bitmask = "110".parse().unwrap();
    let bier_state: BierState = from_value(json).expect("Cannot parse the JSON to BierState");
    println!("This is the bier state: {:?}", bier_state);
}
