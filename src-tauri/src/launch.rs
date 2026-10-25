//! Turns the player's Steam-style launch options into the command that starts the game.
//!
//! `prime-run %command% -screen-fullscreen 0` runs `prime-run <game> -screen-fullscreen 0`;
//! leading `KEY=value` words become environment variables; without `%command%` the words are
//! passed to the game as arguments, as Steam does. The text is split on whitespace and never
//! handed to a shell, so `;`, `|` or `$(...)` have no special meaning.

use std::ffi::OsString;
use std::path::Path;

const COMMAND_MARKER: &str = "%command%";

/// What to run: `program args…` with `env` added to the launcher's own environment.
#[derive(Debug, PartialEq)]
pub struct LaunchCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub env: Vec<(String, String)>,
}

pub fn build(options: &str, exe: &Path) -> LaunchCommand {
    let mut words = options.split_whitespace().peekable();
    let mut env = Vec::new();
    while let Some((key, value)) = words.peek().and_then(|word| env_assignment(word)) {
        env.push((key.to_string(), value.to_string()));
        words.next();
    }
    let rest: Vec<&str> = words.collect();
    let mut argv: Vec<OsString> = if rest.contains(&COMMAND_MARKER) {
        rest.iter()
            .map(|word| if *word == COMMAND_MARKER { exe.into() } else { word.into() })
            .collect()
    } else {
        std::iter::once(exe.into()).chain(rest.iter().map(Into::into)).collect()
    };
    let program = argv.remove(0);
    LaunchCommand { program, args: argv, env }
}

/// `KEY=value` with a shell-style variable name; anything else (e.g. `-flag=1`) is not one.
fn env_assignment(word: &str) -> Option<(&str, &str)> {
    let (key, value) = word.split_once('=')?;
    let mut chars = key.chars();
    let starts_well = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let is_name = starts_well && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    is_name.then_some((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = "/games/FlySim/FlySim";

    fn run(options: &str) -> (String, Vec<String>, Vec<(String, String)>) {
        let launch = build(options, Path::new(EXE));
        let text = |s: OsString| s.into_string().unwrap();
        (text(launch.program), launch.args.into_iter().map(text).collect(), launch.env)
    }

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn empty_options_start_the_game_directly() {
        assert_eq!(run(""), (EXE.into(), vec![], vec![]));
        assert_eq!(run("   "), (EXE.into(), vec![], vec![]));
    }

    #[test]
    fn wrapper_runs_the_game() {
        assert_eq!(run("prime-run %command%"), ("prime-run".into(), vec![EXE.into()], vec![]));
    }

    #[test]
    fn wrappers_and_game_arguments_keep_their_order() {
        let (program, args, _) = run("gamemoderun prime-run %command% -screen-fullscreen 0");
        assert_eq!(program, "gamemoderun");
        assert_eq!(args, ["prime-run", EXE, "-screen-fullscreen", "0"]);
    }

    #[test]
    fn leading_assignments_become_environment() {
        let (program, args, vars) = run("__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia %command%");
        assert_eq!((program.as_str(), args.len()), (EXE, 0));
        assert_eq!(vars, env(&[("__NV_PRIME_RENDER_OFFLOAD", "1"), ("__GLX_VENDOR_LIBRARY_NAME", "nvidia")]));
    }

    #[test]
    fn without_marker_words_are_game_arguments() {
        assert_eq!(
            run("MANGOHUD=1 -screen-fullscreen 0"),
            (EXE.into(), vec!["-screen-fullscreen".into(), "0".into()], env(&[("MANGOHUD", "1")]))
        );
    }

    #[test]
    fn flag_with_equals_is_an_argument_not_a_variable() {
        assert_eq!(run("-quality=high"), (EXE.into(), vec!["-quality=high".into()], vec![]));
        assert_eq!(run("1X=2"), (EXE.into(), vec!["1X=2".into()], vec![]));
    }

    #[test]
    fn shell_syntax_is_passed_through_literally() {
        assert_eq!(run("%command% ;rm"), (EXE.into(), vec![";rm".into()], vec![]));
    }
}
