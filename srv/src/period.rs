#[derive(serde::Deserialize, Eq, PartialEq, Copy, Clone)]
pub enum Period {
    Prepare,
    Call,
    Continuous,
    Suspense,
}
