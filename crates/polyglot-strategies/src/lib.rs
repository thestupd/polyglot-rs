//! Strategies are independent of the built-in format implementations.
mod append;
mod embed;
mod media;
mod overlap;

pub use append::Append;
pub use embed::Embed;
pub use media::{GifCarrier, JpegCarrier, PngCarrier};
pub use overlap::Overlap;
use polyglot_core::{Composer, Registry, Result};

pub fn builtin_composer(registry: Registry, max_output_bytes: usize) -> Result<Composer> {
    let mut composer = Composer::new(registry.clone());
    composer.register_strategy(Append::new(registry, max_output_bytes))?;
    composer.register_strategy(PngCarrier::new(max_output_bytes))?;
    composer.register_strategy(GifCarrier::new(max_output_bytes))?;
    composer.register_strategy(JpegCarrier::new(max_output_bytes))?;
    composer.register_strategy(Embed)?;
    composer.register_strategy(Overlap)?;
    Ok(composer)
}
