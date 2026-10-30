use crate::error::{AppError, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const FORBIDDEN_PROGRAMS: &[&str] = &[
    "cmd",
    "cmd.exe",
    "powershell",
    "powershell.exe",
    "pwsh",
    "pwsh.exe",
    "bash",
    "sh",
    "wscript",
    "cscript",
];

fn program_basename(program: &str) -> String {
    Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(program)
        .to_lowercase()
}

/// Parses a hook command line into program path and arguments (supports quoted paths).
pub fn parse_hook_line(line: &str) -> Result<(String, Vec<String>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(AppError::Other("Hook command cannot be empty".to_string()));
    }

    let parts = shlex::split(trimmed).ok_or_else(|| {
        AppError::Other("Hook command has invalid quoting".to_string())
    })?;

    let mut iter = parts.into_iter();
    let program = iter
        .next()
        .ok_or_else(|| AppError::Other("Hook command has no program".to_string()))?;

    Ok((program, iter.collect()))
}

/// Validates a hook before it is stored in launcher config.
pub fn validate_hook_line(line: &str) -> Result<()> {
    let (program, _args) = parse_hook_line(line)?;
    let path = PathBuf::from(program.trim_matches('"'));

    if !path.is_absolute() {
        return Err(AppError::Other(
            "Hook program must be an absolute path to an executable".to_string(),
        ));
    }

    let base = program_basename(path.to_string_lossy().as_ref());
    if FORBIDDEN_PROGRAMS.iter().any(|blocked| base == *blocked) {
        return Err(AppError::Other(format!(
            "Hook program '{}' is not allowed",
            base
        )));
    }

    if !path.is_file() {
        return Err(AppError::Other(format!(
            "Hook program does not exist: {}",
            path.display()
        )));
    }

    Ok(())
}

/// Spawns a hook process without invoking a shell.
pub fn spawn_hook(line: &str, current_dir: &Path) -> Result<std::process::Child> {
    let (program, args) = parse_hook_line(line)?;
    Command::new(program)
        .args(args)
        .current_dir(current_dir)
        .stdin(Stdio::null())
        .spawn()
        .map_err(AppError::Io)
}
