/// Arbitrary bytes, including opaque formats without a registered handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolyglotInput {
    pub format: String,
    pub data: Vec<u8>,
    pub label: Option<String>,
}
