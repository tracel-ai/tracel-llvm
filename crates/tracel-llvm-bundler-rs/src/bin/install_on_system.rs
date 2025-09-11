use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    tracel_llvm_bundler_rs::bundler::bundle_cache()?;
    Ok(())
}
