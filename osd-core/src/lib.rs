//! osd-core: OpenSequenceDiagrams core library - A sequence diagram parser and SVG renderer
//!
// Suppress some clippy warnings that are stylistic or too strict
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::manual_strip)]
#![allow(clippy::option_map_unit_fn)]
#![allow(clippy::manual_inspect)]
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
