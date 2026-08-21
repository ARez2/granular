/// Just a wrapper around String to be flexible on where we load assets from
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AssetPath(String);

impl AssetPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl From<String> for AssetPath {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl From<&str> for AssetPath {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}
