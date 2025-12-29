//! Keymap parser for evildoer.
//!
//! Provides functionality for parsing keymaps from strings.
//! Defines structures for keys, modifiers, and key combinations.
//!
//! # Examples
//!
//! Parse a keymap string into a `Node`:
//! ```
//! use evildoer_keymap_parser::{parse, Node, Key, Modifier};
//!
//! let node = parse("ctrl-alt-f").unwrap();
//! assert_eq!(node, Node::new(Modifier::Ctrl | Modifier::Alt, Key::Char('f')));
//! ```
//!
//! Parse a key sequence:
//! ```
//! use evildoer_keymap_parser::{parse_seq, Node, Key};
//!
//! let nodes = parse_seq("g g").unwrap();
//! assert_eq!(nodes, vec![Node::from(Key::Char('g')), Node::from(Key::Char('g'))]);
//! ```
pub mod node;
pub mod parser;

pub use node::{Key, Modifier, Modifiers, Node};
pub use parser::{parse, parse_seq};
