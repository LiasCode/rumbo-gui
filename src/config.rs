//! The command line, read once at startup.

use std::path::PathBuf;

pub const USAGE: &str = "\
usage: rumbo-gui [--rumbo PATH] [DIR] [-- FLAGS]
       rumbo-gui --attached [--rumbo PATH]

Opens a window on a rumbo host working in DIR, or asks for a folder when
DIR is left out. FLAGS go to the `rumbo serve` of the first session as
they are, such as -c to continue the latest session, -s ID for another,
or -p PROVIDER and -m MODEL. Each other session opened is a rumbo of its
own, which the window starts.

  --rumbo PATH   the rumbo executable, when it is not on PATH
  --attached     be the front end rumbo starts: register it with
                 `rumbo frontend add gui rumbo-gui --attached --rumbo '{self}'`,
                 then run `rumbo -f gui` in a project
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

const ATTACHED_ALONE: &str =
    "--attached takes nothing else but --rumbo and its path: rumbo gives the folder and its flags";

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Parsed {
    Open(Config),
    /// rumbo started this front end, and gives it the protocol. The path is
    /// the rumbo executable the window starts for the other sessions.
    Attached(PathBuf),
    Help,
}

/// Reads the arguments after the program's name. A folder that is named
/// must exist, so a mistake shows before any window opens.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, String> {
    let args: Vec<String> = args.into_iter().collect();

    if args.iter().any(|arg| arg == "--attached") {
        let rest: Vec<&str> = args
            .iter()
            .map(String::as_str)
            .filter(|arg| *arg != "--attached")
            .collect();

        return match rest.as_slice() {
            [] => Ok(Parsed::Attached("rumbo".into())),
            ["--rumbo", path] => Ok(Parsed::Attached(path.into())),
            _ => Err(ATTACHED_ALONE.into()),
        };
    }

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

/// The flags that start the host of `session`, or of a new one when it
/// names none. What the first start asked for once, such as an objective
/// or `-r`, is not asked again.
pub fn continuing(session: &str) -> Vec<String> {
    match session.is_empty() {
        true => Vec::new(),
        false => vec!["-s".to_string(), session.to_string()],
    }
}

/// The flags that start a host in another project, in the session used
/// last there.
pub fn continuing_latest() -> Vec<String> {
    vec!["-c".to_string()]
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
            parsed(&["--rumbo", "/opt/rumbo", ".", "--", "-c", "-p", "toy"]),
            Ok(Parsed::Open(Config {
                rumbo: "/opt/rumbo".into(),
                dir: Some(here),
                serve_flags: vec!["-c".into(), "-p".into(), "toy".into()],
            }))
        );
    }

    #[test]
    fn continues_the_session_without_asking_again() {
        assert_eq!(continuing("4cf0261c"), ["-s", "4cf0261c"]);
        assert!(continuing("").is_empty());
        assert_eq!(continuing_latest(), ["-c"]);
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
        assert_eq!(
            parsed(&["--attached"]),
            Ok(Parsed::Attached("rumbo".into()))
        );
        assert_eq!(
            parsed(&["--attached", "--rumbo", "/opt/rumbo"]),
            Ok(Parsed::Attached("/opt/rumbo".into()))
        );
        assert_eq!(
            parsed(&["--rumbo", "/opt/rumbo", "--attached"]),
            Ok(Parsed::Attached("/opt/rumbo".into()))
        );
        assert!(
            parsed(&["--attached", "."])
                .unwrap_err()
                .contains("nothing else")
        );
        assert!(parsed(&["--attached", "--rumbo"]).is_err());
    }
}
