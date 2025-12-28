use osd_core::{parse, render};
use std::fs;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let input_file = args.get(1).expect("Usage: gen_svg <input.wsd>");
    let input = fs::read_to_string(input_file).expect("Failed to read input file");
    let diagram = parse(&input).expect("Failed to parse diagram");
    let svg = render(&diagram);
    println!("{}", svg);
}
