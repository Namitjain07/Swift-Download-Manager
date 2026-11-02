//! Model Context Protocol support: lets an AI assistant look at and control downloads.
//!
//! - `protocol`: the JSON-RPC messages and the tool list (no I/O, easy to test)
//! - `command`: what a tool call asks the app to do

// The pieces are wired together in the next commits.
#![allow(dead_code)]

pub mod command;
pub mod protocol;
