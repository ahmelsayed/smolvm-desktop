//! Open a machine's interactive shell in an external terminal emulator.
//!
//! The embedded Exec tab is useful for quick commands, but a native terminal
//! window gives users their own scrollback, fonts, and keybindings. This module
//! builds `smolvm machine shell --name <name>` and launches it in the terminal
//! emulator the user is already running (or the first one we can find).

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use std::process::Command;

use crate::smolvm::cli;

/// Open a machine's interactive shell in a new terminal window.
#[tauri::command]
pub fn open_external_terminal(name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("machine name is empty".to_string());
    }
    let bin = cli::binary_path().ok_or_else(|| {
        format!(
            "`{}` was not found on PATH; set the smolvm binary in Settings",
            cli::current_binary()
        )
    })?;
    spawn_in_terminal(&build_remote_command(&bin, name))
}

fn build_remote_command(bin: &str, name: &str) -> Vec<String> {
    vec![
        bin.to_string(),
        "machine".into(),
        "shell".into(),
        "--name".into(),
        name.to_string(),
    ]
}

/// Build a `Command` with the AppImage's bundled-library environment stripped.
///
/// AppImage's `AppRun` exports `LD_LIBRARY_PATH` and the `GTK_*`/`GDK_*`/`GI_*`
/// module paths pointing at its own bundle. Those must not leak into a host
/// terminal emulator (or the host `smolvm` it launches), which need the host's
/// GTK/glib stack.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn terminal_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    for key in [
        "LD_PRELOAD",
        "DYLD_LIBRARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "APPDIR",
        "APPIMAGE",
        "ARGV0",
        "OWD",
        "GTK_DATA_PREFIX",
        "GTK_THEME",
        "GTK_EXE_PREFIX",
        "GTK_PATH",
        "GTK_IM_MODULE_FILE",
        "GDK_BACKEND",
        "GDK_PIXBUF_MODULE_FILE",
        "GIO_EXTRA_MODULES",
        "GSETTINGS_SCHEMA_DIR",
        "GI_TYPELIB_PATH",
    ] {
        cmd.env_remove(key);
    }
    if let Ok(ld) = std::env::var("LD_LIBRARY_PATH") {
        let appdir = std::env::var("APPDIR").unwrap_or_default();
        let kept: Vec<&str> = ld
            .split(':')
            .filter(|p| {
                !p.is_empty()
                    && !p.starts_with("/tmp/.mount_")
                    && (appdir.is_empty() || !p.starts_with(appdir.as_str()))
            })
            .collect();
        if kept.is_empty() {
            cmd.env_remove("LD_LIBRARY_PATH");
        } else {
            cmd.env("LD_LIBRARY_PATH", kept.join(":"));
        }
    }
    cmd
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn resolve_terminal(name: &str) -> Option<String> {
    let p = Path::new(name);
    if p.is_absolute() || name.contains('/') {
        return if p.is_file() {
            Some(name.to_string())
        } else {
            None
        };
    }
    let path = std::env::var_os("PATH")?;
    cli::which_in(name, &path)
}

/// Spawn a detached child and reap it on a background thread so a long-lived
/// terminal window doesn't leave a zombie when it closes.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn spawn_and_reap(mut cmd: Command) -> Result<(), String> {
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn spawn_terminal(program: &str, remote: &[String]) -> Result<(), String> {
    let base = Path::new(program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(program);
    let inv = terminal_invocation(base);
    let mut cmd = terminal_command(program);
    cmd.args(&inv.prefix);
    if inv.join_command {
        cmd.arg(shell_join(remote));
    } else {
        cmd.args(remote);
    }
    spawn_and_reap(cmd).map_err(|e| format!("failed to launch {program}: {e}"))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Debug, PartialEq)]
struct TerminalInvocation {
    prefix: Vec<String>,
    join_command: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn terminal_invocation(base: &str) -> TerminalInvocation {
    let base = base.to_ascii_lowercase();
    let (prefix, join_command): (Vec<&str>, bool) = match base.as_str() {
        "ghostty"
        | "alacritty"
        | "konsole"
        | "xterm"
        | "urxvt"
        | "st"
        | "tilix"
        | "x-terminal-emulator" => (vec!["-e"], false),
        "wezterm" => (vec!["start", "--"], false),
        "gnome-terminal" => (vec!["--"], false),
        "terminator" => (vec!["-x"], false),
        "xfce4-terminal" => (vec!["--command"], true),
        // kitty, foot and xdg-terminal-exec take the program directly.
        "kitty" | "foot" | "xdg-terminal-exec" => (vec![], false),
        _ => (vec!["-e"], false),
    };
    TerminalInvocation {
        prefix: prefix.into_iter().map(str::to_string).collect(),
        join_command,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|a| shell_quote(a))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '-' | '_' | '.' | '/' | ':' | '=' | '+' | '@' | '%' | ',')
        })
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(target_os = "linux")]
fn spawn_in_terminal(remote: &[String]) -> Result<(), String> {
    let mut tried: Vec<String> = Vec::new();
    let mut last_err: Option<String> = None;
    for candidate in linux_candidates() {
        if tried.iter().any(|t| t == &candidate) {
            continue;
        }
        tried.push(candidate.clone());
        if let Some(term) = resolve_terminal(&candidate) {
            match spawn_terminal(&term, remote) {
                Ok(()) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        format!(
            "no terminal emulator found (tried: {}). Set SMOLVM_DESKTOP_TERMINAL to a terminal executable.",
            tried.join(", ")
        )
    }))
}

#[cfg(target_os = "linux")]
fn linux_candidates() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(v) = std::env::var("SMOLVM_DESKTOP_TERMINAL") {
        if !v.trim().is_empty() {
            out.push(v.trim().to_string());
        }
    }
    if let Ok(v) = std::env::var("TERM_PROGRAM") {
        if let Some(t) = term_program_to_terminal(&v) {
            out.push(t.to_string());
        }
    }
    if let Ok(v) = std::env::var("TERMINAL") {
        if !v.trim().is_empty() {
            out.push(v.trim().to_string());
        }
    }
    for t in [
        "ghostty",
        "kitty",
        "wezterm",
        "alacritty",
        "foot",
        "konsole",
        "gnome-terminal",
        "xfce4-terminal",
        "tilix",
        "terminator",
        "urxvt",
        "st",
        "xterm",
        "xdg-terminal-exec",
        "x-terminal-emulator",
    ] {
        out.push(t.to_string());
    }
    out
}

#[cfg(target_os = "linux")]
fn term_program_to_terminal(program: &str) -> Option<&'static str> {
    match program.to_ascii_lowercase().as_str() {
        "ghostty" => Some("ghostty"),
        "wezterm" => Some("wezterm"),
        "kitty" => Some("kitty"),
        "alacritty" => Some("alacritty"),
        "konsole" => Some("konsole"),
        "xfce4-terminal" => Some("xfce4-terminal"),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn spawn_in_terminal(remote: &[String]) -> Result<(), String> {
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let lower = term_program.to_ascii_lowercase();
    // Ghostty's macOS app explicitly does not support `-e` from the CLI, so
    // only WezTerm gets the CLI path here.
    if lower == "wezterm" {
        if let Some(term) = resolve_terminal("wezterm") {
            return spawn_terminal(&term, remote);
        }
    }

    let cmdline = shell_join(remote);
    let (app, script) = if lower == "iterm.app" {
        (
            "iTerm",
            format!(
                "tell application \"iTerm\"\nactivate\ncreate window with default profile command \"{}\"\nend tell",
                applescript_escape(&cmdline)
            ),
        )
    } else {
        (
            "Terminal",
            format!(
                "tell application \"Terminal\"\nactivate\ndo script \"{}\"\nend tell",
                applescript_escape(&cmdline)
            ),
        )
    };
    let mut cmd = terminal_command("osascript");
    cmd.arg("-e").arg(script);
    spawn_and_reap(cmd).map_err(|e| format!("failed to launch {app} via osascript: {e}"))
}

#[cfg(target_os = "macos")]
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "windows")]
fn spawn_in_terminal(remote: &[String]) -> Result<(), String> {
    if let Some(wt) = resolve_terminal("wt.exe").or_else(|| resolve_terminal("wt")) {
        let mut cmd = terminal_command(&wt);
        cmd.arg("--").args(remote);
        return spawn_and_reap(cmd).map_err(|e| format!("failed to launch Windows Terminal: {e}"));
    }
    // Fallback: `start` opens a new console window for the console program.
    let mut cmd = terminal_command("cmd");
    cmd.args(["/C", "start", ""]).args(remote);
    spawn_and_reap(cmd).map_err(|e| format!("failed to open a console window: {e}"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn spawn_in_terminal(_remote: &[String]) -> Result<(), String> {
    Err("opening an external terminal is not supported on this platform".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_command_targets_machine_shell() {
        assert_eq!(
            build_remote_command("/usr/bin/smolvm", "vm1"),
            ["/usr/bin/smolvm", "machine", "shell", "--name", "vm1"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn terminal_templates() {
        assert_eq!(
            terminal_invocation("ghostty").prefix,
            vec!["-e".to_string()]
        );
        assert_eq!(terminal_invocation("kitty").prefix, Vec::<String>::new());
        assert!(terminal_invocation("xfce4-terminal").join_command);
        assert_eq!(
            terminal_invocation("wezterm").prefix,
            vec!["start".to_string(), "--".to_string()]
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn shell_quote_escapes_spaces_and_single_quotes() {
        assert_eq!(shell_quote("simple"), "simple");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
