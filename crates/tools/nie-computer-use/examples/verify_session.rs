use std::{env, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let executable = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("missing executable"))?;
    let database = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("missing database"))?;
    let binary_id = args
        .next()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1);
    let session = nie_computer_use::ReSession::open(executable, database, Some(binary_id))?;
    println!("{}", serde_json::to_string_pretty(session.target())?);
    Ok(())
}
