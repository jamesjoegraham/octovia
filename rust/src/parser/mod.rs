//! Diagram parsers.
//!
//! Two surface formats are supported:
//!
//! * **DSL** — line-oriented text format (sequence-first, narrative). See
//!   [`dsl`] for the grammar.
//! * **JSON** — structured input intended for adapter layers and LLM tool
//!   calls. See [`json`].
//!
//! Both produce a [`crate::ast::Diagram`] with insertion-order node lists.

mod dsl;
mod json;

pub use dsl::parse_dsl;
pub use json::parse_json;
