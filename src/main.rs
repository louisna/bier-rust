extern crate log;

use clap::Parser;

use serde_json::{Value, from_reader, from_value};
use bier_rust::bier::BierState;

#[derive(Parser)]
struct Args {
    #[clap(short = 'c', long = "config", value_parser, default_value = "configs/example.json")]
    config: String,
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    let file = std::fs::File::open(args.config).expect("Cannot find the file");
    let json: Value = from_reader(file).expect("Cannot read the JSON content");
    let bier_state: BierState = from_value(json).expect("Cannot parse the JSON to BierState");


}
