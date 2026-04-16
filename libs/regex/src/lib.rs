cfg_select! {
    feature = "plugin" => {
        use koto_plugin as runtime;
    }
    _ => {
        use koto_runtime as runtime;
    }
}
use runtime::Result;
use runtime::derive::{KotoCopy, KotoType};
use runtime::derive::{koto_impl, koto_method};
use runtime::prelude::*;

pub fn make_module() -> KMap {
    let result = KMap::with_type("regex");

    result.add_fn("new", |ctx| match ctx.args() {
        [KValue::Str(pattern)] => Ok(regex_to_value(Regex::new(pattern)?)),
        unexpected => unexpected_args("|String|", unexpected),
    });

    result
}

#[cfg(feature = "plugin")]
koto_plugin::export_plugin!(make_module);

#[derive(Clone, Debug, KotoType, KotoCopy)]
#[koto(runtime = runtime)]
pub struct Regex(regex::Regex);

impl Regex {
    pub fn new(pattern: &str) -> Result<Self> {
        match regex::Regex::new(pattern) {
            Ok(regex) => Ok(Self(regex)),
            Err(error) => runtime_error!(format!("failed to parse regex pattern: {error}")),
        }
    }

    fn is_match_text(&self, text: &str) -> bool {
        self.0.is_match(text)
    }

    fn find_match(&self, text: &str) -> Option<Match> {
        self.0
            .find(text)
            .map(|m| Match::new(text.to_string(), m.start(), m.end()))
    }

    fn find_all_matches(&self, text: &str) -> Option<Matches> {
        let matches: Vec<(usize, usize)> = self
            .0
            .find_iter(text)
            .map(|m| (m.start(), m.end()))
            .collect();

        (!matches.is_empty()).then_some(Matches {
            text: text.to_string(),
            matches,
            last_index: 0,
        })
    }

    fn capture_matches(&self, text: &str) -> Option<Vec<(CaptureKey, Option<Match>)>> {
        let captures = self.0.captures(text)?;
        let source = text.to_string();
        let mut result = Vec::with_capacity(captures.len());

        for (i, (capture, name)) in captures.iter().zip(self.0.capture_names()).enumerate() {
            let key = match name {
                Some(name) => CaptureKey::Name(name.to_string()),
                None => CaptureKey::Index(i as i64),
            };

            let value =
                capture.map(|capture| Match::new(source.clone(), capture.start(), capture.end()));
            result.push((key, value));
        }

        Some(result)
    }

    fn replace_all_text(&self, text: &str, replacement: &str) -> String {
        self.0.replace_all(text, replacement).to_string()
    }

    fn captures_value(&self, text: &str) -> KValue {
        let Some(captures) = self.capture_matches(text) else {
            return KValue::Null;
        };

        let entries = captures
            .into_iter()
            .map(|(key, value)| (key, value.map(match_to_value).unwrap_or(KValue::Null)))
            .collect();

        map_entries_to_value(entries)
    }
}

impl runtime::api::KotoObjectOps<runtime::Backend> for Regex {}

fn capture_key_to_value(key: CaptureKey) -> KValue {
    match key {
        CaptureKey::Name(name) => name.into(),
        CaptureKey::Index(index) => index.into(),
    }
}

fn map_entries_to_value(entries: Vec<(CaptureKey, KValue)>) -> KValue {
    KValue::Map(
        entries
            .into_iter()
            .map(|(key, value)| (capture_key_to_value(key), value))
            .collect(),
    )
}

fn regex_to_value(regex: Regex) -> KValue {
    KObject::from(regex).into()
}

#[derive(Clone, Debug, KotoType, KotoCopy)]
#[koto(runtime = runtime)]
pub struct Matches {
    text: String,
    matches: Vec<(usize, usize)>,
    last_index: usize,
}

impl Matches {
    fn next_match(&mut self) -> Option<Match> {
        if self.last_index >= self.matches.len() {
            self.last_index = 0;
            None
        } else {
            let result = self
                .matches
                .get(self.last_index)
                .map(|(start, end)| Match::new(self.text.clone(), *start, *end));
            self.last_index += 1;
            result
        }
    }
}

fn matches_to_value(matches: Matches) -> KValue {
    KObject::from(matches).into()
}

#[derive(Clone, Debug, KotoType, KotoCopy)]
#[koto(runtime = runtime)]
pub struct Match {
    text: String,
    start: usize,
    end: usize,
}

impl Match {
    fn new(text: String, start: usize, end: usize) -> Self {
        Self { text, start, end }
    }

    fn text_value(&self) -> String {
        self.text
            .get(self.start..self.end)
            .unwrap_or_default()
            .to_string()
    }

    fn koto_range(&self) -> KRange {
        KRange::from(self.start as i64..self.end as i64)
    }

    fn display_string(&self) -> String {
        format!(
            "Match('{}', {}..{})",
            self.text_value(),
            self.start,
            self.end
        )
    }
}

impl runtime::api::KotoObjectOps<runtime::Backend> for Match {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        Match::display(self, ctx)
    }
}

fn match_to_value(match_: Match) -> KValue {
    KObject::from(match_).into()
}

enum CaptureKey {
    Name(String),
    Index(i64),
}

#[koto_impl(runtime = runtime)]
impl Regex {
    #[koto_method]
    fn is_match(&self, text: &str) -> bool {
        self.is_match_text(text)
    }

    #[koto_method]
    fn find(&self, text: &KString) -> KValue {
        self.find_match(text.as_str())
            .map(match_to_value)
            .unwrap_or(KValue::Null)
    }

    #[koto_method]
    fn find_all(&self, text: &KString) -> KValue {
        self.find_all_matches(text.as_str())
            .map(matches_to_value)
            .unwrap_or(KValue::Null)
    }

    #[koto_method]
    fn captures(&self, text: &KString) -> KValue {
        self.captures_value(text.as_str())
    }

    #[koto_method]
    fn replace_all(&self, text: &str, replacement: &str) -> String {
        self.replace_all_text(text, replacement)
    }
}

impl<B: KotoBackend> KotoAccess<B> for Matches {}

impl runtime::api::KotoObjectOps<runtime::Backend> for Matches {
    fn is_iterable(&self) -> Result<IsIterable> {
        Ok(IsIterable::ForwardIterator)
    }

    fn iterator_next(&mut self, _vm: &mut KotoVm) -> Result<Option<KIteratorOutput>> {
        Ok(self
            .next_match()
            .map(|match_| KIteratorOutput::Value(match_to_value(match_))))
    }
}

#[koto_impl(runtime = runtime)]
impl Match {
    #[koto_method]
    fn text(&self) -> String {
        self.text_value()
    }

    #[koto_method]
    fn range(&self) -> KRange {
        self.koto_range()
    }

    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.display_string());
        Ok(())
    }
}
