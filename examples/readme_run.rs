use std::time::Duration;

use bio_tools::run::{CommandSpec, RunLogSpec, run};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = CommandSpec::new("opendde")
        .args(["predict", "input.yaml"])
        .current_dir("work")
        .timeout(Duration::from_secs(600))
        .run_log(RunLogSpec::new("process_executables/run_logs", "opendde").artifact("."));

    let output = run(&command)?;
    println!("{}", output.stdout_lossy());
    Ok(())
}
