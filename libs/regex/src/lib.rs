use koto_runtime::{derive::*, prelude::*, Result};
use std::rc::Rc;

pub fn make_module() -> KMap {
    let result = KMap::with_type("regex");

    result.add_fn("new", |ctx| match ctx.args() {
        [Value::Str(pattern)] => Ok(Regex::new(pattern)?.into()),
        unexpected => type_error_with_slice("a regex pattern as string", unexpected),
    });

    result
}

#[derive(Clone, Debug, KotoType, KotoCopy)]
pub struct Regex(Rc<regex::Regex>);

#[koto_impl(runtime = koto_runtime)]
impl Regex {
    pub fn new(pattern: &str) -> Result<Self> {
        match regex::Regex::new(pattern) {
            Ok(r) => Ok(Self(Rc::new(r))),
            Err(e) => runtime_error!("Failed to parse regex pattern: {e}"),
        }
    }

    #[koto_method]
    fn is_match(&self, args: &[Value]) -> Result<Value> {
        match args {
            [Value::Str(text)] => Ok(self.0.is_match(text).into()),
            unexpected => type_error_with_slice("a string", unexpected),
        }
    }

    #[koto_method]
    fn find_all(&self, args: &[Value]) -> Result<Value> {
        match args {
            [Value::Str(text)] => {
                let matches = self.0.find_iter(text);
                Ok(Matches {
                    text: text.clone(),
                    matches: matches.map(|m| (m.start(), m.end())).collect(),
                    last_index: 0,
                }
                .into())
            }
            unexpected => type_error_with_slice("a string", unexpected),
        }
    }

    #[koto_method]
    fn find(&self, args: &[Value]) -> Result<Value> {
        match args {
            [Value::Str(text)] => {
                let m = self.0.find(text);
                match m {
                    Some(m) => Ok(Match::make_value(text.clone(), m.start(), m.end())),
                    None => Ok(Value::Null),
                }
            }
            unexpected => type_error_with_slice("a string", unexpected),
        }
    }

    #[koto_method]
    fn captures(&self, args: &[Value]) -> Result<Value> {
        match args {
            [Value::Str(text)] => {
                match self.0.captures(text) {
                    Some(captures) => {
                        let mut result = ValueMap::with_capacity(captures.len());

                        for (i, (capture, name)) in
                            captures.iter().zip(self.0.capture_names()).enumerate()
                        {
                            if let Some(capture) = capture {
                                let match_ =
                                    Match::make_value(text.clone(), capture.start(), capture.end());

                                // Insert the match with the capture group's index
                                result.insert(i.into(), match_.clone());

                                if let Some(name) = name {
                                    // Also insert the match with the capture group's name
                                    result.insert(name.into(), match_);
                                }
                            } else {
                                result.insert(i.into(), Value::Null);
                            }
                        }

                        Ok(KMap::from(result).into())
                    }
                    None => Ok(Value::Null),
                }
            }
            unexpected => type_error_with_slice("a string", unexpected),
        }
    }

    #[koto_method]
    fn replace_all(&self, args: &[Value]) -> Result<Value> {
        match args {
            [Value::Str(text), Value::Str(replacement)] => {
                let result = self.0.replace_all(text, replacement.as_str());
                Ok(result.to_string().into())
            }
            unexpected => type_error_with_slice("two strings", unexpected),
        }
    }
}

impl KotoObject for Regex {}

impl From<Regex> for Value {
    fn from(regex: Regex) -> Self {
        KObject::from(regex).into()
    }
}

#[derive(Clone, Debug, KotoType, KotoCopy)]
pub struct Matches {
    text: KString,
    matches: Vec<(usize, usize)>,
    last_index: usize,
}

impl Matches {}

impl KotoLookup for Matches {}

impl KotoObject for Matches {
    fn is_iterable(&self) -> IsIterable {
        IsIterable::ForwardIterator
    }

    fn iterator_next(&mut self, _vm: &mut Vm) -> Option<KIteratorOutput> {
        if self.last_index >= self.matches.len() {
            self.last_index = 0;
            None
        } else {
            let result = match self.matches.get(self.last_index) {
                Some((start, end)) => Some(KIteratorOutput::Value(Match::make_value(
                    self.text.clone(),
                    *start,
                    *end,
                ))),
                None => None,
            };

            self.last_index += 1;
            result
        }
    }
}

impl From<Matches> for Value {
    fn from(matches: Matches) -> Self {
        KObject::from(matches).into()
    }
}

#[derive(Clone, Debug, KotoType, KotoCopy)]
pub struct Match {
    text: KString,
    bounds: KRange,
}

#[koto_impl(runtime = koto_runtime)]
impl Match {
    pub fn make_value(matched: KString, start: usize, end: usize) -> Value {
        let Some(text) = matched.with_bounds(start..end) else {
            return Value::Null;
        };

        Self {
            text,
            bounds: KRange::bounded(start as i64, end as i64, false),
        }
        .into()
    }

    #[koto_method]
    fn text(&self) -> Value {
        self.text.clone().into()
    }

    #[koto_method]
    fn range(&self) -> Value {
        self.bounds.clone().into()
    }
}

impl KotoObject for Match {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("Match('{}', {})", self.text, self.bounds));
        Ok(())
    }
}

impl From<Match> for Value {
    fn from(match_: Match) -> Self {
        KObject::from(match_).into()
    }
}
