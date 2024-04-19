use std::{env::current_dir, io::Write, path::PathBuf, str::FromStr};

use anyhow::Result;
use clap::{value_parser, Arg, ArgMatches, Command, Subcommand};
use duct::cmd;
use indoc::formatdoc;

fn main() -> Result<()> {
    let path_str = current_dir()?.as_os_str().to_string_lossy().to_string();

    let cmd = Command::new("cozy")
        .subcommand(init(path_str))
        .subcommand(Command::new("build"))
        .subcommand(Command::new("run"));
    matches(cmd)?;
    Ok(())
}

#[derive(Debug)]
struct File {
    name: String,
    dir_path: PathBuf,
    file: std::fs::File,
}

impl File {
    fn new(name: String, dir_path: PathBuf, contents: String) -> Result<Self> {
        let mut file = Self::touch(&dir_path.join(name.clone()))?;
        file.write(contents.as_bytes())?;
        Ok(Self {
            name,
            dir_path,
            file,
        })
    }
    fn touch(path: &PathBuf) -> Result<std::fs::File> {
        std::fs::File::create(path).map_err(Into::into)
    }
}

#[derive(Debug)]
struct Dir;

#[derive(Debug)]
struct Project {
    dune_project: File,
    opam_file: File,
    bin: Option<Dir>,
    package_json: serde_json::Value,
}

impl Project {
    fn new(name: String, dir_path: PathBuf) -> Result<Self> {
        fn create_dune_project(name: &String, dir_path: &PathBuf) -> Result<File> {
            fn dune_project(name: String) -> Result<String> {
                let output: String = formatdoc! {r#"
                (lang dune 3.14)

                (name {name})

                (generate_opam_files true)

                (source
                (github username/reponame))

                (authors "Author Name")

                (maintainers "Maintainer Name")

                (license LICENSE)

                (documentation https://url/to/documentation)

                (package
                (name {name})
                (synopsis "A short synopsis")
                (description "A longer description")
                (depends ocaml dune)
                (tags
                (topics "to describe" your project)))

                ; See the complete stanza docs at https://dune.readthedocs.io/en/stable/dune-files.html#dune-project
            "#};

                Ok(output)
            }
            let dune_project_contents = dune_project(name.clone())?;
            File::new(name.clone(), dir_path.clone(), dune_project_contents)
        }
        fn create_opam_file(name: &String, dir_path: &PathBuf) -> Result<File> {
            let mut opam_file = String::new();
            opam_file.push_str(name.clone().as_str());
            opam_file.push_str(".opam");
            File::new(opam_file, dir_path.clone(), String::new())
        }
        fn create_bin_dir(dir_path: &PathBuf) -> Result<File> {
            let bin_dir = dir_path.join("bin");
            mkdir(&bin_dir)?;
            // main.ml file inside bin
            fn main_ml() -> Result<&'static str> {
                Ok(r#"let () = print_endline "Hello, World!""#)
            }
            let main_ml_contents = main_ml()?.into();
            File::new("main.ml".into(), bin_dir, main_ml_contents)
        }
        create_bin_dir(&dir_path)?;
        Ok(Self {
            dune_project: create_dune_project(&"dune-project".to_string(), &dir_path)?,
            opam_file: create_opam_file(&name, &dir_path)?,
            bin: None,
            package_json: serde_json::value::Value::default(),
        })
    }
}

fn matches(cmd: Command) -> Result<()> {
    let mut cmd_ = cmd.clone();
    let binding = cmd.get_matches();

    let name: String = match binding.subcommand() {
        Some(("init", matches)) => matches
            .get_one::<String>("name")
            .into_iter()
            .map(|x| x.clone())
            .collect(),
        None => {
            cmd_.print_long_help()?;
            "".to_string()
        }
        _ => unreachable!("Somehow the error for name arg didn't work so Idk what happened"),
    };
    match binding.subcommand() {
        Some(("init", matches)) => {
            let path: PathBuf = matches.get_one::<PathBuf>("path").into_iter().collect();

            // create a file in current_dir + name
            let dir_path = path.clone().join(PathBuf::from(name.clone()));
            if !dir_path.exists() {
                mkdir(&dir_path)?;
            }

            Project::new(name, dir_path)?;
        }

        Some(("build", _)) => {
            // esy ocamlfind ocamlc -package <| packages |> -linkpkg  <| all the stuff in bin
            // directory
            let esy = "esy";
            let ocamlfind = "ocamlfind";
            let ocamlc = "ocamlc";
            let mut packages: Vec<String> = vec![];
            let package_flag = "-package";
            let linkpkg_flag = "-linkpkg";
            let cwd = current_dir()?;
            let bin_dir = cwd.join("bin").join("main.ml");

            let cmd = cmd!(
                esy,
                ocamlfind,
                ocamlc,
                package_flag,
                packages.join(","),
                linkpkg_flag,
                bin_dir
            );

            ()
        }
        None => (),
        _ => unreachable!("This is not a subcommand or you shouldn't be here"),
    };

    Ok(())
}

fn init(path_str: String) -> impl Into<Command> {
    Command::new("init")
        .arg(
            Arg::new("path")
                .long("path")
                .short('p')
                .require_equals(false)
                .default_value(path_str)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(Arg::new("name").required(true))
}

fn mkdir(dir: &PathBuf) -> Result<()> {
    std::fs::create_dir(dir).map_err(Into::into)
}
