//! End-to-end tests that run the compiled `collatz` binary.

use std::process::{Command, Output};

fn collatz(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_collatz"))
        .args(args)
        .output()
        .expect("failed to run the collatz binary")
}

fn stdout(args: &[&str]) -> String {
    let output = collatz(args);
    assert!(
        output.status.success(),
        "collatz {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn orbit_of_27() {
    let out = stdout(&["orbit", "27"]);
    assert!(out.contains("27  reaches 1 in 111 steps"));
    assert!(out.contains("(41 odd, 70 even)"));
    assert!(out.contains("9,232"));
    assert!(out.contains("59 shortcut steps"));
}

#[test]
fn orbit_of_a_huge_number() {
    let out = stdout(&["orbit", "2^200-1", "--map", "syracuse", "--height", "4"]);
    assert!(out.contains("(61 digits)"));
    assert!(out.contains("syracuse map"));
}

#[test]
fn full_orbit_listing() {
    let out = stdout(&["orbit", "6", "--full", "--height", "2"]);
    assert!(out.contains("6 3 10 5 16 8 4 2 1"));
}

#[test]
fn verify_reports_known_values() {
    let out = stdout(&["verify", "1e6", "--threads", "3", "--sieve", "16"]);
    assert!(out.contains("✓ every n < 1,000,000 reaches 1"));
    // 626331 holds the stopping-time record below a million: 287 standard
    // steps, of which 111 are odd, so 176 shortcut steps.
    assert!(out.contains("626,331 needs 176 steps"));
}

#[test]
fn verify_checksum_ignores_thread_count() {
    let checksum = |threads: &str| {
        stdout(&["verify", "2^26", "-t", threads])
            .lines()
            .find(|line| line.contains("checksum"))
            .unwrap()
            .to_string()
    };
    assert_eq!(checksum("1"), checksum("4"));
}

#[test]
fn records_and_delays() {
    let out = stdout(&["records", "1e5"]);
    assert!(out.contains("77,671") && out.contains("1,570,824,736"));
    let out = stdout(&["delays", "1e6"]);
    assert!(out.contains("837,799") && out.contains("524"));
}

#[test]
fn cycles_and_patterns() {
    let out = stdout(&["cycles", "--range", "500"]);
    assert!(out.contains("1 → 4 → 2"));
    assert!(out.contains("-5 → -14 → -7 → -20 → -10"));
    assert!(out.contains("No orbit escaped"));
    let out = stdout(&[
        "cycles",
        "--a",
        "5",
        "--b",
        "1",
        "--positive",
        "--range",
        "50",
    ]);
    assert!(out.contains("13 → 66"));
    assert!(out.contains("escaped"));
    let out = stdout(&["patterns", "12"]);
    assert!(out.contains("L = 11  -17 → -25"));
}

#[test]
fn minus_verification_and_basins() {
    let out = stdout(&["verify", "1e6", "--minus", "-k", "16"]);
    assert!(out.contains("reaches one of the cycles through 1, 5 and 17"));
    let out = stdout(&["basins", "2^16", "--bits", "80", "--samples", "4000"]);
    // Exact shares below 2^16, from the table.
    assert!(out.contains("33.074% / 32.319% / 34.607%"));
    assert!(out.contains("n ≈ 2^80"));
}

#[test]
fn sieve_table() {
    let out = stdout(&["sieve", "--depth", "10"]);
    assert!(out.contains("1,024") && out.contains("64"));
}

#[test]
fn plot_writes_svg() {
    let svg = stdout(&["plot", "orbits", "27", "97"]);
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("peak 9,232"));
    let svg = stdout(&["plot", "stopping", "1000"]);
    assert!(svg.contains("871 takes 178 steps"));
}

#[test]
fn bad_input_exits_with_usage_error() {
    for args in [
        &["frobnicate"][..],
        &["verify"],
        &["verify", "2^200"],
        &["orbit", "0"],
        &["orbit", "27", "--nope"],
        &["plot", "pie"],
    ] {
        let output = collatz(args);
        assert_eq!(output.status.code(), Some(2), "collatz {args:?}");
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("error:"));
    }
}

#[test]
fn step_limit_is_a_failure_not_a_hang() {
    let output = collatz(&["verify", "1000", "--max-steps", "10"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("did not drop below itself"));
}

#[test]
fn help() {
    let out = stdout(&["help"]);
    for command in [
        "orbit", "verify", "records", "delays", "sieve", "cycles", "patterns", "bench", "plot",
    ] {
        assert!(
            out.contains(&format!("  {command}")),
            "help is missing {command}"
        );
    }
}
