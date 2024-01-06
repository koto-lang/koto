use koto_runtime::{derive::*, prelude::*, Result};
use std::collections::HashMap;
use std::rc::Rc;

pub fn make_module() -> KMap {
    let result = KMap::with_type("re");
    result.add_fn("regex", |ctx| match ctx.args() {
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
                    text: Rc::from(text.as_str()),
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
                    Some(m) => Ok(Match::new(Rc::from(text.as_str()), m.start(), m.end()).into()),
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
                let captures = self.0.captures(text);
                let capture_names = self.0.capture_names();
                match captures {
                    Some(captures) => {
                        let mut byname = HashMap::new();
                        for name in capture_names.flatten() {
                            let m = captures.name(name).unwrap();
                            byname.insert(Rc::from(name), (m.start(), m.end()));
                        }

                        Ok(Captures {
                            text: Rc::from(text.as_str()),
                            captures: captures
                                .iter()
                                .map(|m| m.map(|m| (m.start(), m.end())))
                                .collect(),
                            byname,
                        }
                        .into())
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
    text: Rc<str>,
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
                Some((start, end)) => Some(KIteratorOutput::Value(
                    Match::new(self.text.as_ref().into(), *start, *end).into(),
                )),
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
    text: Rc<str>,
    start: usize,
    end: usize,
}

#[koto_impl(runtime = koto_runtime)]
impl Match {
    pub fn new(text: Rc<str>, start: usize, end: usize) -> Self {
        Self { text, start, end }
    }

    #[koto_method]
    fn text(&self) -> Value {
        (self.text[self.start..self.end]).into()
    }

    #[koto_method]
    fn start(&self) -> Value {
        self.start.into()
    }

    #[koto_method]
    fn end(&self) -> Value {
        self.end.into()
    }

    #[koto_method]
    fn range(&self) -> Value {
        KRange::bounded(
            self.start.try_into().unwrap(),
            self.end.try_into().unwrap(),
            false,
        )
        .into()
    }
}

impl KotoObject for Match {}

impl From<Match> for Value {
    fn from(match_: Match) -> Self {
        KObject::from(match_).into()
    }
}

#[derive(Clone, Debug, KotoType, KotoCopy)]
pub struct Captures {
    text: Rc<str>,
    captures: Vec<Option<(usize, usize)>>,
    byname: HashMap<Rc<str>, (usize, usize)>,
}

#[koto_impl(runtime = koto_runtime)]
impl Captures {
    #[koto_method]
    fn get(&self, args: &[Value]) -> Result<Value> {
        match args {
            [Value::Number(index)] => match self.captures.get(index.as_i64() as usize) {
                Some(Some((start, end))) => Ok(Match::new(self.text.clone(), *start, *end).into()),
                _ => Ok(Value::Null),
            },
            [Value::Str(name)] => match self.byname.get(name.as_str()) {
                Some(m) => Ok(Match::new(self.text.clone(), m.0, m.1).into()),
                None => Ok(Value::Null),
            },
            unexpected => type_error_with_slice("a number", unexpected),
        }
    }

    #[koto_method]
    fn len(&self) -> Value {
        self.captures.len().into()
    }
}

impl KotoObject for Captures {
    fn index(&self, index: &Value) -> Result<Value> {
        match index {
            Value::Number(index) => match self.captures.get(index.as_i64() as usize) {
                Some(Some((start, end))) => Ok(Match::new(self.text.clone(), *start, *end).into()),
                _ => runtime_error!("Invalid capture group index"),
            },
            Value::Str(name) => match self.byname.get(name.as_str()) {
                Some(m) => Ok(Match::new(self.text.clone(), m.0, m.1).into()),
                None => runtime_error!("Invalid capture group name"),
            },
            unexpected => type_error("Invalid index (must be Number or Str)", unexpected),
        }
    }
}

impl From<Captures> for Value {
    fn from(captures: Captures) -> Self {
        KObject::from(captures).into()
    }
}
