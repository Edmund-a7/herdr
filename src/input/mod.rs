mod encode;
mod model;
mod parse;

pub use encode::{encode_key, encode_terminal_key};
pub use model::{KeyboardProtocol, TerminalKey};
pub use parse::parse_terminal_key_sequence;

#[cfg(test)]
mod tests;
