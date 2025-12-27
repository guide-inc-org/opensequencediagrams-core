//! osd-core: OpenSequenceDiagrams core library - A sequence diagram parser and SVG renderer
//!
//! # Example
//!
//! ```
//! use osd_core::{parse, render};
//!
//! let input = r#"
//! title Example
//! Alice->Bob: Hello
//! Bob-->Alice: Hi there
//! "#;
//!
//! let diagram = parse(input).unwrap();
//! let svg = render(&diagram);
//! println!("{}", svg);
//! ```
//!
//! # Themed rendering
//!
//! ```
//! use osd_core::{parse, render_with_config, Config, Theme};
//!
//! let input = "Alice->Bob: Hello";
//! let diagram = parse(input).unwrap();
//! let config = Config::default().with_theme(Theme::modern_blue());
//! let svg = render_with_config(&diagram, config);
//! ```

pub mod ast;
pub mod parser;
pub mod renderer;
pub mod theme;

pub use ast::*;
pub use parser::{parse, ParseError};
pub use renderer::{render, render_with_config, Config};
pub use theme::{LifelineStyle, ParticipantShape, Theme};
