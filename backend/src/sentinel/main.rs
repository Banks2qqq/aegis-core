use std::error::Error;

/// Minimal entrypoint for the `sentinel-core` binary.
///
/// In the current repo layout, the primary operational binary is `agent-cli`.
/// `sentinel-core` is kept as a stable placeholder so the workspace builds cleanly.
fn main() -> Result<(), Box<dyn Error>> {
    println!("sentinel-core: OK");
    Ok(())
}

