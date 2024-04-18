#![forbid(unsafe_code)]

// things that didn't work to make this faster:
// - using an array instead of a vec for state.path
// - using get_unchecked_mut to look up most recent key in path

// things that did work to make this faster:
// - directly writing to the newest key in path directly over the 2nd newest,
//   in the case that we are inside an object and moving from one key to the next,
//   rather than popping the 2nd newest and pushing the newest

use aws_smithy_json::deserialize::{JsonTokenIterator, Token};
use path_value_writer::PathValueWriter;

pub mod path_value_writer;

pub type Path<'input> = &'input [PathComponent<'input>];

#[derive(Clone, Copy, Debug)]
pub enum PathComponent<'input> {
    Key(aws_smithy_json::deserialize::EscapedStr<'input>),
    Index(usize),
}

/// represents the "atomic" JSON datatypes,
/// meaning all types that are not collections
///
/// TODO should we also include empty object ({})
/// and empty array ([]) as atoms?
/// would these be useful to show in path output?
/// if so we could do it by storing the previous
/// token, and checking `}` and `]` to see if the previous
/// token was `{` or `[`.
pub enum JsonAtom<'input> {
    String(aws_smithy_json::deserialize::EscapedStr<'input>),
    Null,
    Bool(bool),
    Number(aws_smithy_types::Number),
}

#[derive(Default)]
struct State<'input> {
    /// the current path, in order from least deep to most deep, i.e.,
    /// `{"a": {"b": {"c": 1}}}`
    /// corresponds to:
    /// `/a/b/c  1`
    path: Vec<PathComponent<'input>>,
    /// how deep we are in the object hierarchy, i.e.,
    /// `{"a": {"b": {"c": 1}}}`
    /// has depth = 3
    depth: usize,
}

impl<'input> State<'input> {
    fn pop_path(&mut self) {
        self.path.pop();
    }

    fn add_new_array_index_to_path(&mut self) {
        self.path.push(PathComponent::Index(0));
    }

    fn increment_depth(&mut self) {
        self.depth = self
            .depth
            .checked_add(1)
            .expect("object depth must not exceed a usize")
    }

    fn decrement_depth(&mut self) {
        self.depth = self
            .depth
            .checked_sub(1)
            .expect("object depth must not be negative, this is a bug")
    }

    /// if the most recent path component is an array index, increment its value
    fn maybe_increment_most_recent_array_index(&mut self) {
        if let Some(PathComponent::Index(i)) = self.path.last_mut() {
            *i = i
                .checked_add(1)
                .expect("array length must not exceed usize")
        }
    }

    /// example:
    /// {"a": {"b": 1, "c": 2}}
    ///
    /// for "a":
    /// - current depth is 1
    /// - current path length is 0,
    /// - so, push a new key
    ///
    /// for "b":
    /// - current depth is 2
    /// - current path length is 1
    /// - so, push a new key
    ///
    /// for "c":
    /// - current depth is 2
    /// - current path length is 2
    /// - so, write "c" to `path` index 1
    ///
    /// which results in paths for values of:
    /// /a/b 1
    /// /a/c 2
    fn add_new_object_key_to_path(
        &mut self,
        key: aws_smithy_json::deserialize::EscapedStr<'input>,
    ) {
        // if this method is called (in the `ObjectKey` branch),
        // that means we have already hit a `StartObject` token,
        // (as object keys must be within objects),
        // so, `depth` must be >= 1,
        // so, in order for `depth` to be <= `path.len()` in this comparison,
        // `path.len()` must also be >= 1.
        if self.depth <= self.path.len() {
            // I benchmarked `last_mut` against `get_unchecked_mut`,
            // and there was no apparent different in throughput,
            // so we will stay with this because it avoids unsafe code and the worst
            // case is that it panics
            if let Some(last) = self.path.last_mut() {
                *last = PathComponent::Key(key)
            } else {
                unreachable!()
            }
        // otherwise, if `depth` > `path.len()`,
        // we just push the key rather than write to an existing index
        } else {
            self.path.push(PathComponent::Key(key));
        }
    }
}

// TODO:
// should tokens be moved or &mut?
pub fn stream<W: PathValueWriter>(
    writer: &mut W,
    tokens: JsonTokenIterator,
) -> std::io::Result<()> {
    let mut state = State::default();

    for token in tokens {
        let token = token.map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        match token {
            Token::ValueString { value, .. } => {
                writer.write_path_and_value(&state.path, JsonAtom::String(value))?;
            }
            Token::ValueNumber { value, .. } => {
                writer.write_path_and_value(&state.path, JsonAtom::Number(value))?;
            }
            Token::ValueBool { value, .. } => {
                writer.write_path_and_value(&state.path, JsonAtom::Bool(value))?;
            }
            Token::ValueNull { .. } => {
                writer.write_path_and_value(&state.path, JsonAtom::Null)?;
            }
            Token::ObjectKey { key, .. } => {
                state.add_new_object_key_to_path(key);
            }
            Token::StartObject { .. } => state.increment_depth(),
            Token::StartArray { .. } => {
                state.increment_depth();
                state.add_new_array_index_to_path()
            }
            Token::EndObject { .. } | Token::EndArray { .. } => {
                state.decrement_depth();
                state.pop_path();
            }
        }

        if is_terminal(&token) {
            state.maybe_increment_most_recent_array_index()
        }
    }

    Ok(())
}

fn is_terminal(token: &Token) -> bool {
    matches!(
        token,
        Token::ValueString { .. }
            | Token::ValueNumber { .. }
            | Token::ValueBool { .. }
            | Token::ValueNull { .. }
            | Token::EndObject { .. }
            | Token::EndArray { .. }
    )
}

#[cfg(test)]
mod tests {
    use crate::path_value_writer::json_pointer::{
        Options as JSONPointerWriterOptions, Writer as JSONPointerWriter,
    };
    use aws_smithy_json::deserialize::json_token_iter;

    use super::*;

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
}
