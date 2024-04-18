#[cfg(feature = "jemalloc")]
#[global_allocator]
static ALLOC: jemalloc::Jemalloc = jemalloc::Jemalloc;

use clap::Parser;
use jstream::path_value_writer::json_pointer::{
    Options as JSONPointerWriterOptions, Writer as JSONPointerWriter,
};
use std::error::Error;
use std::io::{BufWriter, Read};
use std::mem::ManuallyDrop;
use std::path::PathBuf;

/// Enumerate the paths through a JSON document.
#[derive(Parser, Debug)]
#[clap(author, version, about, name = "jstream")]
struct Options {
    /// A JSON file path
    #[arg()]
    json_location: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    // https://github.com/rust-lang/rust/issues/46016
    #[cfg(target_family = "unix")]
    {
        use nix::sys::signal;
        let _ = unsafe { signal::signal(signal::Signal::SIGPIPE, signal::SigHandler::SigDfl)? };
    }

    let options = Options::parse();

    let buf = if let Some(json_location) = &options.json_location {
        std::fs::read(json_location)?
    } else {
        let mut buf = vec![];
        let stdin = std::io::stdin();
        let mut stdin = stdin.lock();
        stdin.read_to_end(&mut buf)?;
        buf
    };

    let buf = ManuallyDrop::new(buf);

    // TODO should tokens also be ManuallyDrop?
    // it doesn't seem to show up in benchmarks when run against citylots.json
    let tokens = aws_smithy_json::deserialize::json_token_iter(&buf);

    let mut stdout = BufWriter::new(std::io::stdout().lock());

    let mut json_pointer_writer =
        JSONPointerWriter::new(&mut stdout, JSONPointerWriterOptions::default());

    jstream::stream(&mut json_pointer_writer, tokens)?;

    Ok(())
}
