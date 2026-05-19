use std::{process::Command, thread::sleep};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration, Local, NaiveTime};
use clap::Parser;
use log::{info, warn};
use wait_timeout::ChildExt;
use which::which;

#[derive(Parser)]
struct UserArgs {
    /// Time specification
    #[arg(short, long)]
    time: String,

    /// quiet, don't display the expected timeout
    #[arg(short, long)]
    quiet: bool,

    /// restart
    #[arg(short, long)]
    restart: bool,

    /// delay before restarting
    #[arg(long, default_value_t = 10)]
    restart_delay: u64,

    /// Command
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 1..)]
    command: Vec<String>,
}

impl UserArgs {
    pub fn command_line(&self) -> String {
        self.command.join(" ")
    }
}

fn time_until(target: &NaiveTime) -> (i64, i64, i64) {
    let now = Local::now().time();
    let mut diff = *target - now; // chrono::Duration
    if diff < Duration::zero() {
        diff += Duration::days(1); // assume tomorrow
    }
    let total = diff.num_seconds();
    (total / 3600, (total % 3600) / 60, total % 60)
}

fn parse_time(time_spec: &str) -> Result<NaiveTime> {
    //
    // Try rfc3339 first
    //
    if let Ok(dt) = DateTime::parse_from_rfc3339(time_spec) {
        return Ok(dt.time());
    }

    //
    // just hours:minutes:seconds parsing, if it's before now, we'll wrap
    //
    if let Ok(dt) = NaiveTime::parse_from_str(time_spec, "%H:%M:%S") {
        return Ok(dt);
    }

    //
    // just hours:minutes parsing, if it's before now, we'll wrap
    //
    if let Ok(dt) = NaiveTime::parse_from_str(time_spec, "%H:%M") {
        return Ok(dt);
    }

    bail!("Unknown time spec format");
}

fn run_until(seconds: f64, command_line: &[String]) -> Result<()> {
    let program = command_line
        .first()
        .ok_or_else(|| anyhow!("program is missing"))?;

    let program = which(program).with_context(|| format!("Unable to find {program} in PATH"))?;

    info!("program: {}", program.display());

    let args = command_line.iter().skip(1);

    let mut child = Command::new(&program)
        .args(args)
        .spawn()
        .with_context(|| format!("Unable to spawn {}", program.display()))?;

    let timeout = std::time::Duration::from_secs_f64(seconds);

    let status = match child.wait_timeout(timeout)? {
        Some(v) => v.code().unwrap_or(-1),
        None => {
            warn!("{} timed out, signaling", program.display());
            child.kill().context("Unable to kill sub process")?;
            let code = child.wait().context("Wait failure")?;
            code.code().unwrap_or(-1)
        }
    };

    info!("{} returned {status}", program.display());

    Ok(())
}

fn run(args: &UserArgs) -> Result<()> {
    let timeout = parse_time(&args.time)
        .with_context(|| format!("Unable to parse time spectification \"{}\"", args.time))?;

    let now = Local::now().time();

    let diff = if timeout > now {
        timeout - now
    } else {
        timeout - now + Duration::days(1)
    };

    if !args.quiet {
        let (hours, minutes, seconds) = time_until(&timeout);
        println!("hours={hours} minutes={minutes} seconds={seconds}");
    }

    info!("timeout in seconds: {}", diff.as_seconds_f64());
    info!("command line:       \"{}\"", args.command_line());

    run_until(diff.as_seconds_f64(), &args.command)
}

fn main() -> Result<()> {
    let args = UserArgs::parse();

    env_logger::init();

    if args.restart {
        loop {
            run(&args)?;
            info!("sleeping for {} seconds", args.restart_delay);
            sleep(std::time::Duration::from_secs(args.restart_delay))
        }
    } else {
        run(&args)
    }
}
