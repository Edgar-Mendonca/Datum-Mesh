//! The core every module shares: the sparse solver, the eigen solver, load cases and
//! combinations, and the path solver (Newton and arc length) that nonlinear steps run on.

pub mod cases;
pub mod dynamics;
pub mod frames;
pub mod modal;
pub mod path;
pub mod sparse;
