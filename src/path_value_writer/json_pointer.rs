use super::PathValueWriter;
use crate::{JsonAtom, Path, PathComponent};
use std::io::Write;

pub struct Writer<'writer, W: Write> {
    writer: &'writer mut W,
    options: Options<'writer>,
}

impl<'writer, W: Write> Writer<'writer, W> {
    pub fn new(writer: &'writer mut W, options: Options<'writer>) -> Self {
        Self { writer, options }
    }
}

pub struct Options<'options> {
    separator: &'options str,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self { separator: "\t" }
    }
}

impl<'writer, W: Write> PathValueWriter for Writer<'writer, W> {
    fn write_path_and_value(&mut self, path: Path, value: JsonAtom) -> std::io::Result<()> {
        write_path(self.writer, path)?;

        self.writer.write_all(self.options.separator.as_bytes())?;

        match value {
            JsonAtom::String(s) => {
                self.writer.write_all(b"\"")?;
                self.writer.write_all(s.as_escaped_str().as_bytes())?;
                self.writer.write_all(b"\"")?;
            }
            JsonAtom::Null => {
                self.writer.write_all(b"null")?;
            }
            JsonAtom::Bool(b) => {
                if b {
                    self.writer.write_all(b"true")?;
                } else {
                    self.writer.write_all(b"false")?;
                }
            }
            JsonAtom::Number(n) => match n {
                aws_smithy_types::Number::PosInt(i) => {
                    let mut b = itoa::Buffer::new();
                    self.writer.write_all(b.format(i).as_bytes())?;
                }
                aws_smithy_types::Number::NegInt(i) => {
                    let mut b = itoa::Buffer::new();
                    self.writer.write_all(b.format(i).as_bytes())?;
                }
                aws_smithy_types::Number::Float(f) => {
                    let mut b = ryu::Buffer::new();
                    self.writer.write_all(b.format(f).as_bytes())?;
                }
            },
        }

        self.writer.write_all(b"\n")?;

        Ok(())
    }
}

fn write_path<W: Write>(writer: &mut W, path_components: &[PathComponent]) -> std::io::Result<()> {
    for item in path_components {
        writer.write_all(b"/")?;

        let mut b = itoa::Buffer::new();
        let as_bytes = match item {
            // TODO test this with keys that need to be escaped,
            // we may need to use the escaped form. not clear.
            PathComponent::Key(k) => k.as_escaped_str().as_bytes(),
            PathComponent::Index(index) => b.format(*index).as_bytes(),
        };
        writer.write_all(as_bytes)?;
    }

    Ok(())
}
