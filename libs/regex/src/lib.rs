use koto_runtime::{derive::*, prelude::*, Ptr, Result};

pub fn make_module() -> KMap {
    let result = KMap::with_type("regex");

    result.add_fn("new", |ctx| match ctx.args() {
        [KValue::Str(pattern)] => Ok(Regex::new(pattern)?.into()),
        unexpected => unexpected_args("|String|", unexpected),
    });

    result
}

#[derive(Clone, Debug, KotoType, KotoCopy)]
pub struct Regex(Ptr<regex::Regex>);

#[koto_impl(runtime = koto_runtime)]
impl Regex {
    pub fn new(pattern: &str) -> Result<Self> {
        match regex::Regex::new(pattern) {
            Ok(r) => Ok(Self(r.into())),
            Err(e) => runtime_error!("Failed to parse regex pattern: {e}"),
        }
    }

    #[koto_method]
    fn is_match(&self, args: &[KValue]) -> Result<KValue> {
        match args {
            [KValue::Str(text)] => Ok(self.0.is_match(text).into()),
            unexpected => unexpected_args("|String|", unexpected),
        }
    }

    #[koto_method]
    fn find(&self, args: &[KValue]) -> Result<KValue> {
        match args {
            [KValue::Str(text)] => {
                let m = self.0.find(text);
                match m {
                    Some(m) => Ok(Match::make_value(text.clone(), m.start(), m.end())),
                    None => Ok(KValue::Null),
                }
            }
            unexpected => unexpected_args("|String|", unexpected),
        }
    }

    #[koto_method]
    fn find_all(&self, args: &[KValue]) -> Result<KValue> {
        match args {
            [KValue::Str(text)] => {
                let matches: Vec<(usize, usize)> = self
                    .0
                    .find_iter(text)
                    .map(|m| (m.start(), m.end()))
                    .collect();

                let result = if matches.is_empty() {
                    KValue::Null
                } else {
                    Matches {
                        text: text.clone(),
                        matches,
                        last_index: 0,
                    }
                    .into()
                };

                Ok(result)
            }
            unexpected => unexpected_args("|String|", unexpected),
        }
    }

    #[koto_method]
    fn captures(&self, args: &[KValue]) -> Result<KValue> {
        match args {
            [KValue::Str(text)] => {
                match self.0.captures(text) {
                    Some(captures) => {
                        let mut result = ValueMap::with_capacity(captures.len());

                        for (i, (capture, name)) in
                            captures.iter().zip(self.0.capture_names()).enumerate()
                        {
                            if let Some(capture) = capture {
                                let match_ =
                                    Match::make_value(text.clone(), capture.start(), capture.end());

                                if let Some(name) = name {
                                    // Also insert the match with the capture group's name
                                    result.insert(name.into(), match_);
                                } else {
                                    // Insert the match with the capture group's index
                                    result.insert(i.into(), match_);
                                }
                            } else {
                                result.insert(i.into(), KValue::Null);
                            }
                        }

                        Ok(KMap::from(result).into())
                    }
                    None => Ok(KValue::Null),
                }
            }
            unexpected => unexpected_args("|String|", unexpected),
        }
    }

    #[koto_method]
    fn replace_all(&self, args: &[KValue]) -> Result<KValue> {
        match args {
            [KValue::Str(text), KValue::Str(replacement)] => {
                let result = self.0.replace_all(text, replacement.as_str());
                Ok(result.to_string().into())
            }
            unexpected => unexpected_args("|String, String|", unexpected),
        }
    }
}

impl KotoObject for Regex {}

impl From<Regex> for KValue {
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

impl KotoEntries for Matches {}

impl KotoObject for Matches {
    fn is_iterable(&self) -> IsIterable {
        IsIterable::ForwardIterator
    }

    fn iterator_next(&mut self, _vm: &mut KotoVm) -> Option<KIteratorOutput> {
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

impl From<Matches> for KValue {
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
    pub fn make_value(matched: KString, start: usize, end: usize) -> KValue {
        let Some(text) = matched.with_bounds(start..end) else {
            return KValue::Null;
        };

        Self {
            text,
            bounds: KRange::from(start as i64..end as i64),
        }
        .into()
    }

    #[koto_method]
    fn text(&self) -> KValue {
        self.text.clone().into()
    }

    #[koto_method]
    fn range(&self) -> KValue {
        self.bounds.clone().into()
    }
}

impl KotoObject for Match {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("Match('{}', {})", self.text, self.bounds));
        Ok(())
    }
}

impl From<Match> for KValue {
    fn from(match_: Match) -> Self {
        KObject::from(match_).into()
    }
}
