//! Runs recognition engines in worker processes, so an engine that crashes or hangs takes
//! down only its worker, and a stuck recognition can be stopped by ending the worker.

mod client;
mod host;
mod wire;
mod worker;

pub use client::{WorkerBackend, WorkerCommand, isolated_backends};
pub use host::{reply_output, serve};
