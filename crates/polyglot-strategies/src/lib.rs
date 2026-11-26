//! Strategies are independent of the built-in format implementations.
mod append;
mod embed;
mod overlap;

pub use append::Append;
pub use embed::Embed;
pub use overlap::Overlap;
use polyglot_core::{Composer, Registry, Result};

pub fn builtin_composer(registry: Registry, max_output_bytes: usize) -> Result<Composer> {
    let mut composer = Composer::new(registry.clone());
    composer.register_strategy(Append::new(registry, max_output_bytes))?;
    composer.register_strategy(Embed)?;
    composer.register_strategy(Overlap)?;
    Ok(composer)
}
