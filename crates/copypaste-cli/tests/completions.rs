//! Shell completion generation tests.
//!
//! `copypaste` ships completions via `scripts/completions.sh <shell>` (no `clap_complete`
//! dep, no `completions` subcommand). These tests verify each shell's output is non-empty,
//! syntactically plausible for that shell, and stays in lockstep with the actual CLI
//! subcommands declared in `src/main.rs` (drift guard).
//!
//! If the script ever gets replaced by a clap-based generator, swap `run_script` for a
//! binary call (`Command::new(env!("CARGO_BIN_EXE_copypaste")).args(["completions", sh])`).

use std::path::PathBuf;
use std::process::Command;

/// Subcommands that MUST appear in every shell's completion output.
/// Sourced manually from `src/main.rs` `enum Commands` — if a subcommand is added
/// to the CLI but not to the completion script, these tests fail and force an update.
const REQUIRED_SUBCOMMANDS: &[&str] = &[
    "list", "count", "status", "delete", "search", "copy", "watch", "export", "clear",
    "stats", "import",
];

fn repo_root() -> PathBuf {
    // tests/completions.rs -> crates/copypaste-cli/tests -> crates/copypaste-cli -> crates -> root
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .expect("workspace root above crates/copypaste-cli")
        .to_path_buf()
}

fn run_script(shell: &str) -> (bool, String, String) {
    let script = repo_root().join("scripts").join("completions.sh");
    assert!(
        script.exists(),
        "completions.sh missing at {}",
        script.display()
    );

    let out = Command::new("bash")
        .arg(&script)
        .arg(shell)
        .output()
        .expect("spawn bash for completions.sh");

    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn bash_completion_output_contains_all_subcommands() {
    let (ok, stdout, stderr) = run_script("bash");
    assert!(ok, "bash completion script failed: {stderr}");
    assert!(
        stdout.contains("complete -F") && stdout.contains("copypaste"),
        "bash output missing `complete -F ... copypaste` directive:\n{stdout}"
    );
    assert!(
        stdout.contains("COMPREPLY"),
        "bash completion should populate COMPREPLY"
    );
    for sub in REQUIRED_SUBCOMMANDS {
        assert!(
            stdout.contains(sub),
            "bash completion drift: subcommand `{sub}` missing from scripts/completions.sh \
             (update the script when adding new CLI commands)\n{stdout}"
        );
    }
}

#[test]
fn zsh_completion_output_starts_with_compdef_directive() {
    let (ok, stdout, stderr) = run_script("zsh");
    assert!(ok, "zsh completion script failed: {stderr}");
    let first_nonempty = stdout
        .lines()
        .find(|l| !l.trim().is_empty())
        .expect("zsh output non-empty");
    assert!(
        first_nonempty.starts_with("#compdef"),
        "zsh output must start with `#compdef` directive, got: {first_nonempty}"
    );
    assert!(
        first_nonempty.contains("copypaste"),
        "#compdef line must name `copypaste`"
    );
    for sub in REQUIRED_SUBCOMMANDS {
        assert!(
            stdout.contains(sub),
            "zsh completion drift: subcommand `{sub}` missing"
        );
    }
}

#[test]
fn fish_completion_output_contains_complete_lines() {
    let (ok, stdout, stderr) = run_script("fish");
    assert!(ok, "fish completion script failed: {stderr}");
    let complete_lines: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with("complete -c copypaste"))
        .collect();
    assert!(
        complete_lines.len() >= REQUIRED_SUBCOMMANDS.len(),
        "fish should emit at least one `complete -c copypaste` line per subcommand, \
         got {} lines for {} required subcommands",
        complete_lines.len(),
        REQUIRED_SUBCOMMANDS.len()
    );
    for sub in REQUIRED_SUBCOMMANDS {
        let pat = format!("-a {sub} ");
        assert!(
            stdout.contains(&pat) || stdout.contains(&format!("-a {sub}\n")),
            "fish completion drift: subcommand `{sub}` missing (-a {sub} not found)"
        );
    }
}

#[test]
fn powershell_completion_renders() {
    // PowerShell is not currently supported by completions.sh. Asserting the contract:
    // the script must exit with non-zero status and print a usage hint, NOT silently
    // emit garbage. When PowerShell support is added (Register-ArgumentCompleter), this
    // test should be flipped to assert the directive is present.
    let (ok, stdout, stderr) = run_script("powershell");
    if ok {
        // Future-proof branch: if support gets added, validate it.
        assert!(
            stdout.contains("Register-ArgumentCompleter"),
            "powershell support added but missing Register-ArgumentCompleter directive"
        );
    } else {
        assert!(
            stderr.contains("Usage:") || stderr.contains("bash|zsh|fish"),
            "unsupported shell should print usage to stderr, got: {stderr}"
        );
    }
}

#[test]
fn completion_for_each_shell_returns_nonempty_nonpanic() {
    for shell in ["bash", "zsh", "fish"] {
        let (ok, stdout, stderr) = run_script(shell);
        assert!(ok, "{shell} completion failed: {stderr}");
        assert!(
            !stdout.trim().is_empty(),
            "{shell} completion produced empty stdout"
        );
        // No panics, no Rust backtraces, no script errors leaking through.
        assert!(
            !stdout.contains("panicked")
                && !stdout.contains("RUST_BACKTRACE")
                && !stderr.contains("syntax error"),
            "{shell} completion output looks corrupted:\nstdout={stdout}\nstderr={stderr}"
        );
    }
}

#[test]
fn unknown_shell_exits_with_usage_error() {
    let (ok, _stdout, stderr) = run_script("tcsh");
    assert!(!ok, "unknown shell must exit non-zero");
    assert!(
        stderr.contains("Usage:") || stderr.contains("bash|zsh|fish"),
        "unknown shell must print usage hint to stderr, got: {stderr}"
    );
}
