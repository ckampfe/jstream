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
    write_empty_collections: bool,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            separator: "\t",
            write_empty_collections: false,
        }
    }
}

impl<'writer, W: Write> PathValueWriter for Writer<'writer, W> {
    fn write_path_and_value(&mut self, path: Path, value: JsonAtom) -> std::io::Result<()> {
        match value {
            JsonAtom::String(s) => {
                write_path(self.writer, path)?;
                self.writer.write_all(self.options.separator.as_bytes())?;
                self.writer.write_all(b"\"")?;
                self.writer.write_all(s.as_escaped_str().as_bytes())?;
                self.writer.write_all(b"\"")?;
                self.writer.write_all(b"\n")?;
            }
            JsonAtom::Null => {
                write_path(self.writer, path)?;
                self.writer.write_all(self.options.separator.as_bytes())?;
                self.writer.write_all(b"null")?;
                self.writer.write_all(b"\n")?;
            }
            JsonAtom::Bool(b) => {
                write_path(self.writer, path)?;
                self.writer.write_all(self.options.separator.as_bytes())?;

                if b {
                    self.writer.write_all(b"true")?;
                } else {
                    self.writer.write_all(b"false")?;
                }

                self.writer.write_all(b"\n")?;
            }
            JsonAtom::Number(n) => {
                write_path(self.writer, path)?;
                self.writer.write_all(self.options.separator.as_bytes())?;

                match n {
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
                }

                self.writer.write_all(b"\n")?;
            }
            JsonAtom::EmptyObject => {
                if self.options.write_empty_collections {
                    write_path(self.writer, path)?;
                    self.writer.write_all(self.options.separator.as_bytes())?;
                    self.writer.write_all(b"{}")?;
                    self.writer.write_all(b"\n")?;
                }
            }
            JsonAtom::EmptyArray => {
                if self.options.write_empty_collections {
                    write_path(self.writer, path)?;
                    self.writer.write_all(self.options.separator.as_bytes())?;
                    self.writer.write_all(b"[]")?;
                    self.writer.write_all(b"\n")?;
                }
            }
        }

        Ok(())
    }
}

fn write_path<W: Write>(writer: &mut W, path_components: &[PathComponent]) -> std::io::Result<()> {
    for item in path_components {
        writer.write_all(b"/")?;

        match item {
            // TODO test this with keys that need to be escaped,
            // we may need to use the escaped form. not clear.
            PathComponent::Key(k) => {
                let as_bytes = k.as_escaped_str().as_bytes();
                writer.write_all(as_bytes)?;
            }
            PathComponent::Index(index) => {
                let mut b = itoa::Buffer::new();
                let as_bytes = b.format(*index).as_bytes();
                writer.write_all(as_bytes)?;
            }
        };
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Options as JSONPointerWriterOptions, Writer as JSONPointerWriter};
    use crate::stream;
    use aws_smithy_json::deserialize::json_token_iter;

    #[test]
    fn simple_object() {
        let s = b"{\"a\":1, \"b\":5, \"c\":9}";

        let tokens = json_token_iter(s);

        let mut buf = vec![];

        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        let challenge = b"/a\t1\n/b\t5\n/c\t9\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn simple_array() {
        let s = b"[1,2,3,null,true,false,\"ok\"]";
        let tokens = json_token_iter(s);

        let mut buf = vec![];

        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        let challenge = b"/0\t1\n/1\t2\n/2\t3\n/3\tnull\n/4\ttrue\n/5\tfalse\n/6\t\"ok\"\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn simple_nested_object() {
        let s = b"{\"a\":{\"b\":{\"c\":99}}}";
        let tokens = json_token_iter(s);
        let mut buf = vec![];

        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        let challenge = b"/a/b/c\t99\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn simple_nested_array() {
        let s = b"[1,[2,[3]]]";
        // /0 1
        // /1/0 2
        // /1/1/0 3
        // 0
        // 20
        // 220
        let tokens = json_token_iter(s);
        let mut buf = vec![];

        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        let challenge = b"/0\t1\n/1/0\t2\n/1/1/0\t3\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn nested_array_nulls() {
        let s = b"[null, [null], null, [null, null]]";
        let tokens = json_token_iter(s);
        let mut buf = vec![];

        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        let challenge = b"/0\tnull\n/1/0\tnull\n/2\tnull\n/3/0\tnull\n/3/1\tnull\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn weird_nested_objects_and_arrays() {
        let s = br#"{"a":[{"b":[1,2,3]}]"#;
        let tokens = json_token_iter(s);
        let mut buf = vec![];
        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        let challenge = b"/a/0/b/0\t1\n/a/0/b/1\t2\n/a/0/b/2\t3\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn one_json_jindex() {
        let s = std::fs::read_to_string("fixtures/one.json").unwrap();
        let jindex = std::fs::read_to_string("fixtures/jindex_one.txt").unwrap();

        let tokens = json_token_iter(s.as_bytes());
        let mut buf = vec![];
        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        let sorted_writer = std::str::from_utf8(&buf).unwrap();
        let mut sorted_writer: Vec<_> = sorted_writer.trim().split('\n').collect();
        sorted_writer.sort();
        let mut sorted_writer = sorted_writer.join("\n");
        sorted_writer.push('\n');

        assert_eq!(sorted_writer.as_bytes(), jindex.as_bytes());
    }

    // #[test]
    // fn github_json_jindex() {}

    #[test]
    fn weird_array() {
        let s = br#"[ [ [ "a", "b", "c" ], [ "d", "e", "f" ], [ "g", "h", "i" ], [ "j", "k", "l" ] ] ]"#;

        let tokens = json_token_iter(s);
        let mut buf = vec![];
        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        let challenge = b"/0/0/0\t\"a\"\n/0/0/1\t\"b\"\n/0/0/2\t\"c\"\n/0/1/0\t\"d\"\n/0/1/1\t\"e\"\n/0/1/2\t\"f\"\n/0/2/0\t\"g\"\n/0/2/1\t\"h\"\n/0/2/2\t\"i\"\n/0/3/0\t\"j\"\n/0/3/1\t\"k\"\n/0/3/2\t\"l\"\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn more_weird() {
        let s = b"{
            \"features\": [
                { \"geometry\": {
                    \"coordinates\": [
                        [
                            [ \"a\", \"b\", \"c\" ],
                            [ \"d\", \"e\", \"f\" ],
                            [ \"g\", \"h\", \"i\" ],
                            [ \"j\", \"k\", \"l\" ]
                        ]
                    ]
                }}
            ] 
        }";

        let tokens = json_token_iter(s);
        let mut buf = vec![];
        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        let challenge = b"/features/0/geometry/coordinates/0/0/0\t\"a\"\n/features/0/geometry/coordinates/0/0/1\t\"b\"\n/features/0/geometry/coordinates/0/0/2\t\"c\"\n/features/0/geometry/coordinates/0/1/0\t\"d\"\n/features/0/geometry/coordinates/0/1/1\t\"e\"\n/features/0/geometry/coordinates/0/1/2\t\"f\"\n/features/0/geometry/coordinates/0/2/0\t\"g\"\n/features/0/geometry/coordinates/0/2/1\t\"h\"\n/features/0/geometry/coordinates/0/2/2\t\"i\"\n/features/0/geometry/coordinates/0/3/0\t\"j\"\n/features/0/geometry/coordinates/0/3/1\t\"k\"\n/features/0/geometry/coordinates/0/3/2\t\"l\"\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn even_more_weird() {
        let s = std::fs::read_to_string("fixtures/city_lots_small.json").unwrap();
        let jindex = std::fs::read_to_string("fixtures/jindex_city_lots_small.txt").unwrap();

        let tokens = json_token_iter(s.as_bytes());
        let mut buf = vec![];
        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        let sorted_writer = std::str::from_utf8(&buf).unwrap();
        let mut sorted_writer: Vec<_> = sorted_writer.trim().split('\n').collect();
        sorted_writer.sort();
        let mut sorted_writer = sorted_writer.join("\n");
        sorted_writer.push('\n');

        assert_eq!(sorted_writer.as_bytes(), jindex.as_bytes());
    }

    #[test]
    fn empty_object_doesnt_mess_up_array() {
        // note the empty object at /d/e/f/0
        let s = br#"
        {
            "a": 1,
            "b": 2,
            "c": ["x", "y", "z"],
            "d": {"e": {"f": [{}, 9, "g"]}}
        }"#;

        let tokens = json_token_iter(s);
        let mut buf = vec![];
        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        println!("{}", std::str::from_utf8(&buf).unwrap());

        let challenge =
            b"/a\t1\n/b\t2\n/c/0\t\"x\"\n/c/1\t\"y\"\n/c/2\t\"z\"\n/d/e/f/1\t9\n/d/e/f/2\t\"g\"\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn empty_array_doesnt_mess_up_array() {
        // note the empty array at /d/e/f/0
        let s = br#"
        {
            "a": 1,
            "b": 2,
            "c": ["x", "y", "z"],
            "d": {"e": {"f": [[], 9, "g"]}}
        }"#;

        let tokens = json_token_iter(s);
        let mut buf = vec![];
        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        println!("{}", std::str::from_utf8(&buf).unwrap());

        let challenge =
            b"/a\t1\n/b\t2\n/c/0\t\"x\"\n/c/1\t\"y\"\n/c/2\t\"z\"\n/d/e/f/1\t9\n/d/e/f/2\t\"g\"\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn empty_object_doesnt_mess_up_object() {
        // note the empty object at /d/e/f/g
        let s = br#"
        {
            "a": 1,
            "b": 2,
            "c": ["x", "y", "z"],
            "d": {"e": {"f": {"g": {}}}}
        }"#;

        let tokens = json_token_iter(s);
        let mut buf = vec![];
        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        println!("{}", std::str::from_utf8(&buf).unwrap());

        let challenge = b"/a\t1\n/b\t2\n/c/0\t\"x\"\n/c/1\t\"y\"\n/c/2\t\"z\"\n";

        assert_eq!(buf, challenge);
    }

    #[test]
    fn empty_array_doesnt_mess_up_object() {
        // note the empty object at /d/e/f/g
        let s = br#"
        {
            "a": 1,
            "b": 2,
            "c": ["x", "y", "z"],
            "d": {"e": {"f": {"g": []}}}
        }"#;

        let tokens = json_token_iter(s);
        let mut buf = vec![];
        let mut writer = JSONPointerWriter::new(&mut buf, JSONPointerWriterOptions::default());

        stream(&mut writer, tokens).unwrap();

        println!("{}", std::str::from_utf8(&buf).unwrap());

        let challenge = b"/a\t1\n/b\t2\n/c/0\t\"x\"\n/c/1\t\"y\"\n/c/2\t\"z\"\n";

        assert_eq!(buf, challenge);
    }
}
