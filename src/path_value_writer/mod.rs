use crate::{JsonAtom, Path};

pub mod json_pointer;

pub trait PathValueWriter {
    fn write_path_and_value(&mut self, path: Path, value: JsonAtom) -> std::io::Result<()>;
}
