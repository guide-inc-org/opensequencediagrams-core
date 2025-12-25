use guideline_core::{parse, render};
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: render_file <input_file>");
        std::process::exit(1);
    }

    let input = fs::read_to_string(&args[1]).expect("Failed to read file");

    match parse(&input) {
        Ok(diagram) => {
            let svg = render(&diagram);
            println!("{}", svg);
        }
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    }
}
