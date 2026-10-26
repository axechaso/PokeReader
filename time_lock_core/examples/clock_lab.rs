//! Offline clock laboratory; no game code or 3DS is needed.
use std::{
    env, fs,
    io::{self, BufRead, Write},
    time::Instant,
};
use time_lock_core::{
    clock::Clock,
    date::DateTime,
    preset::{Preset, TITLES},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut preset = Preset::new(TITLES[6].0);
    preset.target = DateTime::parse("2026-10-26T12:00:00.000").unwrap();
    if let Some(path) = args
        .iter()
        .position(|a| a == "--preset")
        .and_then(|i| args.get(i + 1))
    {
        let bytes = fs::read(path)?;
        let title = args
            .iter()
            .position(|a| a == "--title")
            .and_then(|i| args.get(i + 1))
            .map(|s| u64::from_str_radix(s, 16))
            .transpose()?
            .unwrap_or(preset.title_id);
        preset = Preset::parse(&bytes, title).map_err(|e| format!("Invalid preset: {e:?}"))?;
    }
    let deterministic = args.iter().any(|a| a == "--deterministic");
    let started = Instant::now();
    let mut simulated_tick = 0u64;
    let mut clock = Clock::new(1000).unwrap();
    clock
        .start_session(preset.target.to_epoch_ms().unwrap(), preset.initial_mode, 0)
        .unwrap();
    println!(
        "Offline preview only. Title: {:016X}; target: {}",
        preset.title_id, preset.target
    );
    println!(
        "read | freeze | release | step <ms> | retry | target <local ISO> | save <path> | quit"
    );
    if deterministic {
        println!("Deterministic mode: tick <absolute ms> advances the input clock.");
    }
    for line in io::stdin().lock().lines() {
        let line = line?;
        let (command, argument) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
        let tick = if deterministic {
            simulated_tick
        } else {
            started.elapsed().as_millis() as u64
        };
        let result = match command {
            "quit" | "exit" => break,
            "read" => clock.read(tick),
            "freeze" => clock.freeze(tick),
            "release" => clock.release(tick),
            "step" => match argument.parse() {
                Ok(delta) => clock.step(delta),
                Err(_) => {
                    println!("Invalid milliseconds");
                    continue;
                }
            },
            "retry" => clock
                .start_session(
                    preset.target.to_epoch_ms().unwrap(),
                    preset.initial_mode,
                    tick,
                )
                .and_then(|_| clock.read(tick)),
            "tick" if deterministic => {
                match argument.parse() {
                    Ok(value) => simulated_tick = value,
                    Err(_) => {
                        println!("Invalid tick");
                        continue;
                    }
                }
                clock.read(simulated_tick)
            }
            "target" => {
                match DateTime::parse(argument) {
                    Ok(date) => preset.target = date,
                    Err(e) => {
                        println!("Rejected: {e:?}");
                        continue;
                    }
                }
                println!(
                    "Next retry target: {} (current preview unchanged)",
                    preset.target
                );
                continue;
            }
            "save" => {
                fs::write(argument, preset.to_string())?;
                println!("Saved disabled preset: {argument}");
                continue;
            }
            _ => {
                println!("Unknown command");
                continue;
            }
        };
        match result {
            Ok(ms) => println!(
                "{} | {:?} | session {}",
                DateTime::from_epoch_ms(ms).unwrap(),
                clock.mode(),
                clock.session()
            ),
            Err(e) => println!("Rejected: {e:?}"),
        }
        io::stdout().flush()?;
    }
    Ok(())
}
