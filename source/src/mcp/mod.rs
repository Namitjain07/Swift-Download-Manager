//! Model Context Protocol support: lets an AI assistant look at and control downloads.
//!
//! - `protocol`: the JSON-RPC messages, the tool list, resources and prompts (no I/O, easy to test)
//! - `command`: what a tool call asks the app to do
//! - `error`: the kinds of failure an assistant is told about
//! - `policy`: what an assistant may do (access level, sites, private addresses, limits) and how outside text is cleaned
//! - `probe`: looks at a link without downloading it
//! - `audit`: the record of what assistants did
//! - `control`: the private, token-protected door into the open window
//! - `server`: the `--mcp` stdio loop

pub mod audit;
pub mod command;
pub mod control;
pub mod error;
pub mod lock;
pub mod policy;
pub mod probe;
pub mod protocol;
pub mod server;
pub mod setup;

#[cfg(test)]
mod handoff_tests;
