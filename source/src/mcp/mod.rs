//! Model Context Protocol support: lets an AI assistant look at and control downloads.
//!
//! - `protocol`: the JSON-RPC messages and the tool list (no I/O, easy to test)
//! - `command`: what a tool call asks the app to do
//! - `control`: the private, token-protected door into the open window
//! - `server`: the `--mcp` stdio loop

pub mod command;
pub mod control;
pub mod lock;
pub mod protocol;
pub mod server;

#[cfg(test)]
mod handoff_tests;
