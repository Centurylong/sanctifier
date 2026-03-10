use std::process::Command;

pub fn run_kani(unwind: Option<u32>) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new("cargo");

    cmd.arg("kani");

    if let Some(bound) = unwind {
        cmd.arg("--unwind");
        cmd.arg(bound.to_string());
    }

    let status = cmd.status()?;

    if !status.success() {
        return Err("Kani verification failed".into());
    }

    Ok(())
}
