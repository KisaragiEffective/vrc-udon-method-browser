#![deny(clippy::all)]
#![warn(clippy::nursery)]

mod model;
mod parse;
mod query;

pub use model::{MethodRecord, UdonType};
pub use parse::{
    ParseExternError, TypeNameMap, mangle_type_fqcn, parse_extern_symbol,
    parse_extern_symbol_with_types, parse_symbols, parse_symbols_with_types,
};
pub use query::{group_by_declaring_type, search};
