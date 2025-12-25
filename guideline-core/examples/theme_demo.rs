use guideline_core::{parse, render_with_config, Config, Theme};
use std::fs;

fn main() {
    let input = r#"
title Theme Demo

actor User
participant Server
participant Database

User->Server: Request
Server->Database: Query
Database-->Server: Result
Server-->User: Response

note over Server: Processing...

opt Cache hit
    Server-->User: Cached Response
end
"#;

    let themes = [
        ("default", Theme::default_theme()),
        ("modern-blue", Theme::modern_blue()),
        ("modern-green", Theme::modern_green()),
        ("rose", Theme::rose()),
        ("napkin", Theme::napkin()),
        ("earth", Theme::earth()),
        ("plain", Theme::plain()),
        ("mellow", Theme::mellow()),
        ("blue-outline", Theme::blue_outline()),
        ("warm", Theme::warm()),
        ("gray", Theme::gray()),
    ];

    let diagram = parse(input).expect("Failed to parse");

    for (name, theme) in themes {
        let config = Config::default().with_theme(theme);
        let svg = render_with_config(&diagram, config);

        let filename = format!("examples/theme-{}.svg", name);
        fs::write(&filename, &svg).expect("Failed to write file");
        println!("Generated: {}", filename);
    }

    println!("\nAll themes generated successfully!");
}
