use std::{borrow::Borrow, fmt, hash::Hash, sync::Arc};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id(Arc<String>);

impl Id {
    pub fn new(id: Arc<String>) -> Self {
        Self(id)
    }

    pub fn from_str(id: &str) -> Self {
        Self::new(Arc::new(id.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0.as_ref()
    }
}

impl Borrow<str> for Id {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<Arc<String>> for Id {
    fn borrow(&self) -> &Arc<String> {
        &self.0
    }
}

impl PartialEq<str> for Id {
    fn eq(&self, s: &str) -> bool {
        self.as_str() == s
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
