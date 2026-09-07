use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SurfaceArg { NieExe, Ghidra }

pub fn run(surface: SurfaceArg, executable: Option<String>, ghidra_url: String) -> anyhow::Result<()> {
    let surface = match surface { SurfaceArg::NieExe => nie_computer_use::Surface::NieExe, SurfaceArg::Ghidra => nie_computer_use::Surface::Ghidra };
    let request = nie_computer_use::ProbeRequest { surface, executable, ghidra_url };
    println!("{}", nie_computer_use::probe_json(&request)?);
    Ok(())
}
