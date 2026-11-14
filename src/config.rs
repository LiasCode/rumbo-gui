//! The command line, read once at startup.

use std::path::PathBuf;

pub const USAGE: &str = "\
usage: rumbo-gui [--rumbo PATH] [DIR] [-- FLAGS]

Opens a window on a rumbo host working in DIR, or asks for a folder when
DIR is left out. FLAGS go to `rumbo serve` as they are, such as -c to
continue the latest session, -s ID for another, -m MODEL, or -single.

  --rumbo PATH   the rumbo executable, when it is not on PATH
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The rumbo executable to start.
    pub rumbo: PathBuf,
    /// The folder to work in, when the command line named one.
    pub dir: Option<PathBuf>,
    /// The flags for `rumbo serve`.
    pub serve_flags: Vec<String>,
}

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Parsed {
    Open(Config),
    Help,
}

/// Reads the arguments after the program's name. A folder that is named
/// must exist, so a mistake shows before any window opens.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, String> {
    let mut args = args.into_iter();
    let mut rumbo = PathBuf::from("rumbo");
    let mut dir = None;
    let mut serve_flags = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                return Ok(Parsed::Help);
            }

            "--rumbo" => {
                rumbo = args
                    .next()
                    .ok_or("--rumbo needs the path of the rumbo executable")?
                    .into();
            }

            "--" => {
                serve_flags.extend(args.by_ref());
            }

            flag if flag.starts_with('-') => {
                return Err(format!("unknown flag {flag}; flags for rumbo go after --"));
            }

            _ if dir.is_some() => {
                return Err(format!(
                    "only one folder can be opened, and {arg} is a second one"
                ));
            }

            _ => {
                let path = PathBuf::from(&arg);

                if !path.is_dir() {
                    return Err(format!("{arg} is not a folder"));
                }

                dir = Some(
                    path.canonicalize()
                        .map_err(|error| format!("{arg}: {error}"))?,
                );
            }
        }
    }

    Ok(Parsed::Open(Config {
        rumbo,
        dir,
        serve_flags,
    }))
}

/// The flags that start a host again in `session` after the one before it
/// ended. What the first start asked for once, such as an objective or
/// `-r`, is not asked again, and `-single` stays, since it is how the person
/// chose to work.
pub fn continuing(flags: &[String], session: &str) -> Vec<String> {
    let mut again = Vec::new();

    if flags
        .iter()
        .any(|flag| flag == "-single" || flag == "--single")
    {
        again.push("-single".to_string());
    }

    if !session.is_empty() {
        again.extend(["-s".to_string(), session.to_string()]);
    }

    again
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> Result<Parsed, String> {
        parse(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn defaults_to_rumbo_on_path_and_asks_for_a_folder() {
        assert_eq!(
            parsed(&[]),
            Ok(Parsed::Open(Config {
                rumbo: "rumbo".into(),
                dir: None,
                serve_flags: vec![]
            }))
        );
    }

    #[test]
    fn passes_what_follows_the_separator_to_rumbo() {
        let here = std::env::current_dir().unwrap().canonicalize().unwrap();

        assert_eq!(
            parsed(&["--rumbo", "/opt/rumbo", ".", "--", "-c", "-single"]),
            Ok(Parsed::Open(Config {
                rumbo: "/opt/rumbo".into(),
                dir: Some(here),
                serve_flags: vec!["-c".into(), "-single".into()],
            }))
        );
    }

    #[test]
    fn continues_the_session_without_asking_again() {
        let flags: Vec<String> = ["-single", "-r", "-m", "x/y", "fix it"]
            .iter()
            .map(|flag| flag.to_string())
            .collect();

        assert_eq!(
            continuing(&flags, "4cf0261c"),
            ["-single", "-s", "4cf0261c"]
        );
        assert!(continuing(&[], "").is_empty());
    }

    #[test]
    fn refuses_what_it_cannot_use() {
        assert!(parsed(&["--rumbo"]).is_err());
        assert!(parsed(&["-c"]).unwrap_err().contains("after --"));
        assert!(
            parsed(&["/no/such/folder"])
                .unwrap_err()
                .contains("not a folder")
        );
        assert!(parsed(&[".", "."]).unwrap_err().contains("second"));
        assert_eq!(parsed(&["--help"]), Ok(Parsed::Help));
    }
}
