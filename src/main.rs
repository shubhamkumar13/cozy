use std::{env::current_dir, io::Write, path::PathBuf};

use anyhow::Result;
use clap::{value_parser, Arg, Command};
use indoc::formatdoc;

fn main() -> Result<()> {
    let path_str = current_dir()?.as_os_str().to_string_lossy().to_string();

    let cmd = Command::new("cozy").subcommand(
        Command::new("init")
            .arg(
                Arg::new("path")
                    .long("path")
                    .short('p')
                    .require_equals(false)
                    .default_value(path_str)
                    .value_parser(value_parser!(PathBuf)),
            )
            .arg(Arg::new("name").required(true)),
    );

    let binding = cmd.get_matches();
    let name: String = match binding.subcommand() {
        Some(("init", matches)) => matches
            .get_one::<String>("name")
            .into_iter()
            .map(|x| x.clone())
            .collect(),
        _ => unreachable!("Somehow the error for name arg didn't work so Idk what happened"),
    };
    let path: PathBuf = match binding.subcommand() {
        Some(("init", matches)) => matches.get_one::<PathBuf>("path").into_iter().collect(),
        _ => unreachable!("This is not a subcommand or you shouldn't be here"),
    };
    // create a file in current_dir + name
    let dir_path = path.clone().join(PathBuf::from(name.clone()));
    if !dir_path.exists() {
        mkdir(&dir_path)?;
    }
    // create a dune-project
    let dune_project_contents = dune_project(name.clone())?;
    let dune_project_file = dir_path.join("dune_project");
    let mut dune_project_file = touch(&dune_project_file)?;
    // a .opam file
    let mut opam_file = String::new();
    opam_file.push_str(name.clone().as_str());
    opam_file.push_str(".opam");

    let opam_file = touch(&dir_path.join(opam_file))?;
    // if --bin flag then create bin
    let bin_dir = mkdir(&dir_path.join("bin"))?;
    // main.ml file inside bin
    let main_ml_contents = main_ml()?;
    let mut main_ml_file = touch(&dir_path.join("bin").join("main.ml"))?;
    // default is bin + test
    // workspace is bin + lib + test
    // by default lib + test
    //
    // fill up the dune_project and opam file
    dune_project_file.write(dune_project_contents.as_bytes())?;
    main_ml_file.write(main_ml_contents.as_bytes())?;

    Ok(())
}

fn touch(path: &PathBuf) -> Result<std::fs::File> {
    std::fs::File::create(path).map_err(Into::into)
}

fn mkdir(dir: &PathBuf) -> Result<()> {
    std::fs::create_dir(dir).map_err(Into::into)
}

fn cd(path: &PathBuf) -> Result<()> {
    std::env::set_current_dir(path).map_err(Into::into)
}

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

fn main_ml() -> Result<&'static str> {
    Ok(r#"
    let () = print_endline "Hello, World!"
    "#)
}
