use osd_core::{parse, render_svg};
use std::fs;

fn main() {
    let input = fs::read_to_string("/Users/kondomasaki/Documents/osd/opensequencediagrams-web/.claude/stress_test/Ultimate Stress Test.wsd").unwrap();
    let diagram = parse(&input).unwrap();
    let svg = render_svg(&diagram, None);
    fs::write("/Users/kondomasaki/Documents/osd/opensequencediagrams-web/.claude/stress_test/OSD_Ultimate_Stress_Test_CURRENT.svg", svg).unwrap();
    println!("SVG generated successfully");
}
