//! `spec graph --format box`: a left-to-right layered drawing of the document graph.
//!
//! Layers are longest paths from the documents nothing points at; an edge that skips layers
//! runs through a line in each layer between; a cycle edge is listed under the drawing.

mod canvas;
mod draw;
mod layout;

pub use draw::{Drawing, MAX_NODES, Skip, draw};
