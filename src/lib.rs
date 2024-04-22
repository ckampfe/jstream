#![forbid(unsafe_code)]

// things that didn't work to make this faster:
// - using an fixed-length array instead of a vec for state.path
// - using get_unchecked_mut to look up most recent key in path
// - jemalloc. jstream doesn't really do much heap allocation, so jemalloc
//   is not as useful as it was in jindex.

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
/// meaning all types that are leaf nodes in the document tree.
/// this includes empty collections, but not collections
/// which contain elements
pub enum JsonAtom<'input> {
    String(aws_smithy_json::deserialize::EscapedStr<'input>),
    Null,
    Bool(bool),
    Number(aws_smithy_types::Number),
    EmptyObject,
    EmptyArray,
}

#[derive(Debug, Default)]
struct State<'input> {
    /// the current path, in order from least deep to most deep, i.e.,
    /// `{"a": {"b": {"c": 1}}}`
    /// corresponds to:
    /// `/a/b/c  1`
    path: Vec<PathComponent<'input>>,
    /// how deep we are in the document, i.e.,
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
            // for Token::EndObject and Token::EndArray:
            //
            // if depth > state.path.len() here,
            // at the end of an object/array,
            // it means we inside an empty object/array,
            // and should not pop the most recent path,
            // as the most recent path was from the level above,
            // not this level
            Token::EndObject { .. } => {
                writer.write_path_and_value(&state.path, JsonAtom::EmptyObject)?;
                if state.depth <= state.path.len() {
                    state.pop_path()
                }
                state.decrement_depth();
            }
            Token::EndArray { .. } => {
                writer.write_path_and_value(&state.path, JsonAtom::EmptyArray)?;

                if state.depth <= state.path.len() {
                    state.pop_path()
                }
                state.decrement_depth();
            }
        }

        if is_terminal(&token) {
            state.maybe_increment_most_recent_array_index();
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
