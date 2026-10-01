#![deny(clippy::all)]
#![warn(clippy::nursery)]

mod command;

use anyhow::Result;

fn main() -> Result<()> {
    command::run(std::env::args().skip(1))
}
