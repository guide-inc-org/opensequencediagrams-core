use osd_core::{parse, render};

fn main() {
    let input = r#"
participant A
participant B

A->+B: First
B-->-A: End1

state over A: State

A->+B: Second
B-->-A: End2
"#;
    let diagram = parse(input).unwrap();
    let svg = render(&diagram);
    
    println!("=== Activations ===");
    for line in svg.lines() {
        if line.contains("activation") && line.contains("rect") {
            println!("{}", line.trim());
        }
    }
    
    println!("\n=== Arrows ===");
    for line in svg.lines() {
        if line.contains("<line") && line.contains("class=\"message\"") {
            println!("{}", line.trim());
        }
    }
}
