use guideline_core::{parser, renderer};
use std::{env, fs};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: render_file <input.wsd>");
        return;
    }
    let input = fs::read_to_string(&args[1]).expect("Failed to read file");
    match parser::parse(&input) {
        Ok(diagram) => {
            let svg = renderer::render(&diagram);
            println!("{}", svg);
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
        }
    }
}
