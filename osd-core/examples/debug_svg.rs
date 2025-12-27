use osd_core::{parse, render};

fn main() {
    let input = r#"
title Test Diagram

actor User
participant Server
participant Database

User->Server: Request
Server->+Database: Query
Database-->-Server: Result
Server-->User: Response

note over Server: Processing

opt Cache hit
    Server->Server: Return cached
end
"#;

    match parse(input) {
        Ok(diagram) => {
            let svg = render(&diagram);
            println!("{}", svg);
        }
        Err(e) => {
            eprintln!("Parse error: {}", e);
        }
    }
}
