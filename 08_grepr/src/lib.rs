use clap::{App, Arg};
use regex::{Regex, RegexBuilder};
use std::{
    error::Error,
    fs::{self, File},
    io::{self, BufRead, BufReader},
};
use walkdir::WalkDir;

type MyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub struct Config {
    pattern: String,
    files: Vec<String>,
    recursive: bool,
    insensitive: bool,
    count: bool,
    invert_match: bool,
}

// --------------------------------------------------
pub fn get_args() -> MyResult<Config> {
    let matches = App::new("grepr")
        .version("0.1.0")
        .author("Ken Youens-Clark <kyclark@gmail.com>")
        .about("Rust grep")
        .arg(
            Arg::with_name("pattern")
                .value_name("PATTERN")
                .help("Search pattern")
                .required(true),
        )
        .arg(
            Arg::with_name("files")
                .value_name("FILE")
                .help("Input file(s)")
                .required(true)
                .default_value("-")
                .min_values(1),
        )
        .arg(
            Arg::with_name("insensitive")
                .value_name("INSENSITIVE")
                .help("Case-insensitive")
                .short("i")
                .long("insensitive")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("recursive")
                .value_name("RECURSIVE")
                .help("Recursive search")
                .short("r")
                .long("recursive")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("count")
                .value_name("COUNT")
                .help("Count occurrences")
                .short("c")
                .long("count")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("invert")
                .value_name("INVERT")
                .help("Invert match")
                .short("v")
                .long("invert-match")
                .takes_value(false),
        )
        .get_matches();

    Ok(Config {
        pattern: matches.value_of("pattern").unwrap().to_string(),
        files: matches.values_of_lossy("files").unwrap(),
        recursive: matches.is_present("recursive"),
        insensitive: matches.is_present("insensitive"),
        count: matches.is_present("count"),
        invert_match: matches.is_present("invert"),
    })
}

// --------------------------------------------------
pub fn run(config: Config) -> MyResult<()> {
    // println!("{:#?}", config);

    let pattern = RegexBuilder::new(&config.pattern)
        .case_insensitive(config.insensitive)
        .build()
        .map_err(|_| format!("Invalid pattern \"{}\"", &config.pattern))?;

    let entries = find_files(&config.files, config.recursive);
    let num_files = entries.len();
    let print = |fname: &str, val: &str| {
        if num_files > 1 {
            print!("{}:{}", fname, val);
        } else {
            print!("{}", val);
        }
    };

    for entry in entries {
        match entry {
            Err(e) => eprintln!("{}", e),
            Ok(filename) => match open(&filename) {
                Err(e) => eprintln!("{}: {}", filename, e),
                Ok(file) => {
                    match find_lines(file, &pattern, config.invert_match) {
                        Err(e) => eprintln!("{}", e),
                        Ok(matches) => {
                            if config.count {
                                print(
                                    &filename,
                                    &format!("{}\n", &matches.len()),
                                );
                            } else {
                                for line in &matches {
                                    print(&filename, &line);
                                }
                            }
                        }
                    }
                }
            },
        }
    }

    Ok(())
}

// --------------------------------------------------
fn open(filename: &str) -> MyResult<Box<dyn BufRead>> {
    match filename {
        "-" => Ok(Box::new(BufReader::new(io::stdin()))),
        _ => Ok(Box::new(BufReader::new(File::open(filename)?))),
    }
}

// --------------------------------------------------
fn find_lines<T: BufRead>(
    mut file: T,
    pattern: &Regex,
    invert_match: bool,
) -> MyResult<Vec<String>> {
    let mut matches = vec![];
    let mut line = String::new();

    loop {
        let bytes = file.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        if (pattern.is_match(&line) && !invert_match)
            || (!pattern.is_match(&line) && invert_match)
        {
            matches.push(line.clone());
        }
        line.clear();
    }

    Ok(matches)
}

// --------------------------------------------------
fn find_files(files: &[String], recursive: bool) -> Vec<MyResult<String>> {
    let mut results = vec![];

    for path in files {
        match path.as_str() {
            "-" => results.push(Ok(path.to_string())),
            _ => match fs::metadata(&path) {
                Ok(metadata) => {
                    if metadata.is_dir() {
                        if recursive {
                            for entry in WalkDir::new(path)
                                .into_iter()
                                .filter_map(|e| e.ok())
                                .filter(|e| e.file_type().is_file())
                            {
                                results.push(Ok(entry
                                    .path()
                                    .display()
                                    .to_string()));
                            }
                        } else {
                            results.push(Err(From::from(format!(
                                "{} is a directory",
                                path
                            ))));
                        }
                    } else if metadata.is_file() {
                        results.push(Ok(path.to_string()));
                    }
                }
                Err(e) => {
                    results.push(Err(From::from(format!("{}: {}", path, e))))
                }
            },
        }
    }

    results
}

// --------------------------------------------------
#[cfg(test)]
mod test {
    use super::{find_files, find_lines};
    use rand::{distributions::Alphanumeric, Rng};
    use regex::{Regex, RegexBuilder};
    use std::io::Cursor;

    #[test]
    fn test_find_lines() {
        let lines = b"Lorem\nIpsum\r\nDOLOR";

        let re1 = Regex::new("or").unwrap();
        let matches = find_lines(Cursor::new(&lines), &re1, false);
        assert!(matches.is_ok());
        if let Ok(lines) = matches {
            assert_eq!(lines.len(), 1);
        }

        let matches = find_lines(Cursor::new(&lines), &re1, true);
        assert!(matches.is_ok());
        if let Ok(lines) = matches {
            assert_eq!(lines.len(), 2);
        }

        let re2 = RegexBuilder::new("or")
            .case_insensitive(true)
            .build()
            .unwrap();
        let matches = find_lines(Cursor::new(&lines), &re2, false);
        assert!(matches.is_ok());
        if let Ok(lines) = matches {
            assert_eq!(lines.len(), 2);
        }

        let matches = find_lines(Cursor::new(&lines), &re2, true);
        assert!(matches.is_ok());
        if let Ok(lines) = matches {
            assert_eq!(lines.len(), 1);
        }
    }

    #[test]
    fn test_find_files() {
        let files =
            find_files(&vec!["./tests/inputs/fox.txt".to_string()], false);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].as_ref().unwrap(), "./tests/inputs/fox.txt");

        let files = find_files(&["./tests/inputs".to_string()], true);
        assert_eq!(files.len(), 4);

        let bad: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();

        let files = find_files(&vec![bad.clone()], false);
        assert_eq!(files.len(), 1);
        assert!(files[0].is_err());
        assert_eq!(
            files[0].as_ref().unwrap_err().to_string(),
            format!("{}: No such file or directory (os error 2)", &bad)
        );
    }
}