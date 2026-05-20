use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, ExitCode, Stdio},
    thread::sleep,
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration, Local, NaiveTime};
use clap::Parser;
use log::{error, info, warn};
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

fn time_until(target: NaiveTime) -> Result<(i64, i64, i64)> {
    let now = Local::now().time();
    let mut diff = target.signed_duration_since(now); // chrono::Duration
    if diff < Duration::zero() {
        diff = diff
            .checked_add(&Duration::days(1))
            .ok_or_else(|| anyhow!("time warp detected"))?; // assume tomorrow
    }
    let total = diff.num_seconds();
    Ok((total / 3600, (total % 3600) / 60, total % 60))
}

fn print_timeout(program: &Path, target: NaiveTime) -> Result<()> {
    let (hours, minutes, seconds) = time_until(target)?;

    print!("Running \"{}\" for", program.display());

    if hours > 0 {
        print!(" {hours} hour(s)");
    }

    if minutes > 0 {
        print!(" {minutes} minute(s)");
    }

    if seconds > 0 {
        print!(" {seconds} second(s)");
    }

    println!();

    Ok(())
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

fn run_until<I, S>(program: &Path, args: I, timeout: f64, quiet: bool) -> Result<i32>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    info!("program: {}", program.display());

    let mut cmd = Command::new(program);

    cmd.args(args);

    if quiet {
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
    }

    let mut child = cmd.spawn().with_context(|| format!("Unable to spawn {}", program.display()))?;

    let timeout = std::time::Duration::from_secs_f64(timeout);

    let status = if let Some(v) = child.wait_timeout(timeout)? {
        v.code().unwrap_or(-1)
    } else {
        warn!("{} timed out, killing...", program.display());
        child.kill().context("Unable to kill sub process")?;
        let code = child.wait().context("Wait failure")?;
        code.code().unwrap_or(-1)
    };

    info!("{} returned {status}", program.display());

    Ok(status)
}

fn run(args: &UserArgs) -> Result<i32> {
    let timeout =
        parse_time(&args.time).with_context(|| format!("Unable to parse time spectification \"{}\"", args.time))?;

    let now = Local::now().time();

    let diff = if timeout > now {
        timeout.signed_duration_since(now)
    } else {
        timeout
            .signed_duration_since(now)
            .checked_add(&Duration::days(1))
            .ok_or_else(|| anyhow!("time warp detected"))?
    };

    //println!("{} old={} new={}", diff == new_diff, diff, new_diff);

    let program = args.command.first().ok_or_else(|| anyhow!("command line is missing"))?;
    let program = which(program).with_context(|| format!("Unable to find \"{program}\" in PATH"))?;

    if !args.quiet {
        print_timeout(&program, timeout)?;
    }

    info!("timeout in seconds: {}", diff.as_seconds_f64());
    info!("command line:       \"{}\"", args.command.join(" "));

    let command_args = args.command.iter().skip(1);

    run_until(&program, command_args, diff.as_seconds_f64(), args.quiet)
}

fn main() -> Result<ExitCode> {
    let args = UserArgs::parse();

    env_logger::init();

    if args.restart {
        loop {
            run(&args)?;
            info!("sleeping for {} seconds", args.restart_delay);
            sleep(std::time::Duration::from_secs(args.restart_delay));
        }
    } else {
        let exit_code = match run(&args) {
            Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
            Err(e) => {
                error!("{e}");
                ExitCode::from(1)
            }
        };

        Ok(exit_code)
    }
}
