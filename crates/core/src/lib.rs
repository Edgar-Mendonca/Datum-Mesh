//! Datum Mesh engine: the model and its edits, sections, meshing, finite elements, the solver,
//! results and the verification benchmarks. Pure Rust; the window only shows what this computes.

// Numerical code reads best with index loops and explicit NaN-rejecting comparisons.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments, clippy::type_complexity, clippy::neg_cmp_op_on_partial_ord)]

pub mod doc;
pub mod examples;
mod hardening;
pub mod model;
pub mod sections;
