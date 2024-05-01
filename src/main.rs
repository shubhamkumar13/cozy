use std::{
    arch::x86_64::_CMP_EQ_OS,
    env::current_dir,
    io::{Read, Write},
    path::PathBuf,
};

use anyhow::Result;
use clap::{value_parser, Arg, Command};
use duct::cmd;
use hex::FromHex;
use indoc::formatdoc;
use ocaml::ToValue;
use opam_file_rs::value::{OpamFile, OpamFileSection};
use serde_json::json;

ocaml::import! {
    fn hello() -> String
}

#[tokio::main]
async fn main() -> Result<()> {
    let gc = ocaml::runtime::init();
    unsafe {
        println!("This is gc : {:?}", hello(&gc));
    }
    let path_str = current_dir()?.as_os_str().to_string_lossy().to_string();
    fetch("fmt", "0.9.0").await?;
    let path = "/home/sk/rust-projects/cozy/opam".to_string();
    let (checksum, url) = extract_tarball_url_checksum(&path).await?;
    fetch_tarball(url, checksum).await?;

    let cmd = Command::new("cozy")
        .subcommand(init(path_str))
        .subcommand(Command::new("build"))
        .subcommand(Command::new("run"));
    matches(cmd)?;
    Ok(())
}

#[derive(Debug)]
#[allow(dead_code)]
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
#[allow(dead_code)]
struct Dir {
    path: PathBuf,
}

#[derive(Debug)]
#[allow(dead_code)]
struct Project {
    dune_project: File,
    opam_file: File,
    bin: Option<Dir>,
    package_json: File,
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
        fn create_bin_dir(dir_path: &PathBuf) -> Result<Dir> {
            let bin_dir = dir_path.join("bin");
            mkdir(&bin_dir)?;
            // main.ml file inside bin
            fn main_ml() -> Result<&'static str> {
                Ok(r#"let () = print_endline "Hello, World!""#)
            }
            let main_ml_contents = main_ml()?.into();
            let var_name = File::new("main.ml".into(), bin_dir.clone(), main_ml_contents)?;
            Ok(Dir { path: bin_dir })
        }
        fn create_package_json(dir_path: &PathBuf) -> Result<File> {
            let package_json = json! ({
                "dependencies": {
                    "ocaml" : "5.x"
                },
                "devDependencies": {
                    "@opam/ocaml-lsp-server": "*",
                    "@opam/dot-merlin-reader": "*",
                    "@opam/ocamlformat": "*",
                }
            });
            File::new(
                "package.json".to_string(),
                dir_path.clone(),
                package_json.to_string(),
            )
        }
        let bin = create_bin_dir(&dir_path).ok();
        let package_json = create_package_json(&dir_path);
        Ok(Self {
            dune_project: create_dune_project(&"dune-project".to_string(), &dir_path)?,
            opam_file: create_opam_file(&name, &dir_path)?,
            bin,
            package_json: create_package_json(&dir_path)?,
        })
    }
}

fn matches(cmd: Command) -> Result<()> {
    let mut cmd_ = cmd.clone();
    let binding = cmd.get_matches();
    let esy = "esy";
    let ocamlfind = "ocamlfind";
    let ocamlc = "ocamlc";
    let package_flag = "-package";
    let linkpkg_flag = "-linkpkg";

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
        Some(("build", _)) => "".to_string(),
        Some(("run", _)) => "".to_string(),
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

            // 1 - Get the current directory
            let cwd = current_dir()?;
            // 2 - Get the main.ml path
            let main_ml_path = cwd.join("bin").join("main.ml");
            // 3 - Get the package.json path
            let package_json_path = cwd.join("package.json");

            // 4 - Access the dependencies of package.json
            let mut buf = String::new();
            std::fs::File::open(package_json_path)?.read_to_string(&mut buf)?;
            let package_json: serde_json::Value = serde_json::from_str(buf.as_str())?;
            let packages = package_json
                .as_object()
                .map(|x| {
                    x.get("dependencies")
                        .and_then(|v| match v.as_object() {
                            Some(o) => {
                                Some(o.keys().map(ToOwned::to_owned).filter(|x| x != "ocaml"))
                            }
                            None => None,
                        })
                        .expect("Couldn't find keys")
                        .collect::<Vec<String>>()
                })
                .expect("Couldn't find dependencies");

            // 5 - execute the esy install command (which builds a sandbox and installs all the dependencies present in package.json and devDependencies in package.json)
            cmd!(esy, "install").run()?;

            // builds the executable of main.ml file, mostly comprises of all the dependencies and nothing else
            // (TODO but can later be about stuff in lib directory or something else)
            cmd!(
                esy,
                ocamlfind,
                ocamlc,
                package_flag,
                packages.join(","),
                linkpkg_flag,
                main_ml_path
            )
            .run()?;

            ()
        }
        Some(("run", _)) => {
            cmd!(esy, "./a.out").run()?;
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

// const SEQ_LEN: usize = 5;

// fn convert_result_to_string(guess_result: Vec<u8>) -> String {
//     let mut output = std::string::String::new();

//     for (i, num) in guess_result.iter().enumerate() {
//         if *num == 0 {
//             output.push_str("*");
//         } else if *num == 10 {
//             output.push_str("$");
//         } else {
//             // std::write!(output, "{num}").unwrap();
//             std::fmt::write(&mut output, format_args!("{num}")).unwrap();
//         }
//         if i < SEQ_LEN - 1 {
//             output.push(' ');
//         }
//     }

//     output
// }

async fn fetch(pkg_name: &'static str, pkg_version: &'static str) -> Result<(), anyhow::Error> {
    // create package path for opam-repository
    let pkg_path = format!("{}.{}", pkg_name, pkg_version);
    let path = format!(
        "/repos/ocaml/opam-repository/contents/packages/{}/{}/opam",
        pkg_name, pkg_path
    );

    // get the opam file metadata for the above path using octocrab
    let octo = octocrab::instance();
    let opam_file_metadata = octo
        .get::<serde_json::Value, &String, ()>(&path, None::<&()>)
        .await?;

    // convert to a map
    let hmap = opam_file_metadata
        .as_object()
        .expect("object cannot be coerced to a map");

    // get the tarball url
    let download_url = hmap
        .get("download_url")
        .expect("download_url is null")
        .to_string()
        .chars()
        .filter(|x| *x != '\"')
        .collect::<String>();

    // get tarball
    let res = reqwest::get(download_url).await?;
    let json = res.text().await?.clone();
    let json = json.trim_end().to_string();
    let mut file = match std::fs::File::create_new("/home/sk/rust-projects/cozy/opam") {
        Ok(file) => file,
        Err(_) => {
            let _ = std::fs::remove_file("/home/sk/rust-projects/cozy/opam")?;
            let file = std::fs::File::create_new("/home/sk/rust-projects/cozy/opam")?;
            file
        }
    };
    file.write_all(json.as_bytes())?;
    Ok(())
}

async fn extract_tarball_url_checksum(path: &String) -> Result<(String, String), anyhow::Error> {
    let mut opam_file = String::new();
    std::fs::File::open(path)?.read_to_string(&mut opam_file)?;
    let parsed_opam_file = opam_file_rs::parse(&opam_file)?;
    let parsed_opam_file = parsed_opam_file
        .file_contents
        .into_iter()
        .filter(|x| match x {
            opam_file_rs::value::OpamFileItem::Section(_, opam_file_section) => {
                match (
                    &opam_file_section.section_item[0],
                    &opam_file_section.section_item[1],
                ) {
                    (
                        opam_file_rs::value::OpamFileItem::Variable(_, _, _),
                        opam_file_rs::value::OpamFileItem::Variable(_, _, _),
                    ) => true,
                    (
                        opam_file_rs::value::OpamFileItem::Section(_, _),
                        opam_file_rs::value::OpamFileItem::Section(_, _),
                    )
                    | (
                        opam_file_rs::value::OpamFileItem::Section(_, _),
                        opam_file_rs::value::OpamFileItem::Variable(_, _, _),
                    )
                    | (
                        opam_file_rs::value::OpamFileItem::Variable(_, _, _),
                        opam_file_rs::value::OpamFileItem::Section(_, _),
                    ) => false,
                }
            }
            opam_file_rs::value::OpamFileItem::Variable(_, _, _) => false,
        })
        .filter_map(|x| {
            match x {
                opam_file_rs::value::OpamFileItem::Section(_, opam_file_item) => match (&opam_file_item.section_item[0], &opam_file_item.section_item[1]) {
                    (opam_file_rs::value::OpamFileItem::Section(_, _), opam_file_rs::value::OpamFileItem::Section(_, _)) => None,
                    (opam_file_rs::value::OpamFileItem::Section(_, _), opam_file_rs::value::OpamFileItem::Variable(_, _, _)) => None,
                    (opam_file_rs::value::OpamFileItem::Variable(_, _, _), opam_file_rs::value::OpamFileItem::Section(_, _)) => None,
                    (opam_file_rs::value::OpamFileItem::Variable(_, _, c), opam_file_rs::value::OpamFileItem::Variable(_, _, s)) => match (c.clone().kind, s.clone().kind) {
                        (opam_file_rs::value::ValueKind::String(checksum), opam_file_rs::value::ValueKind::String(url)) => Some((checksum, url)),
                        (_, _) => None
                    }
                },
                opam_file_rs::value::OpamFileItem::Variable(_, _, _) => todo!(),
            }
        })
        .collect::<Vec<(String, String)>>()[0].clone();
    Ok((parsed_opam_file.0, parsed_opam_file.1))
}

use anyhow::anyhow;
use sha2::Digest;

use futures_util::StreamExt;

async fn fetch_tarball(url: String, checksum: String) -> Result<(), anyhow::Error> {

    // create directory and file
    let dir_path = "/home/sk/rust-projects/cozy/tarballs/";
    let filename = url.split('/').last().expect("unable to split the url, check if the url is valid or not");
    let file_path = format!("{}/{}", dir_path, filename);

    // check if it exists
    match std::fs::read_dir(dir_path) {
        Ok(_) => { 
        // delete and create 
            std::fs::remove_dir_all(dir_path)?;
            std::fs::create_dir(dir_path)?;
        },
        Err(_) => {
            std::fs::create_dir(dir_path)?;
        },
    };
    let mut file = std::fs::File::create_new(&file_path)?;

    // create a GET request for the URL
    let res = reqwest::get(&url).await?;
    if res.status().is_success() {
        let mut stream = res.bytes_stream();
        // write the fetched stream to the file created above
        loop {
            let Some(Ok(item)) = stream.next().await else {
                println!("Download Complete!");
                break
            };
            file.write(&item)?;
        };
        Ok(())
    } else {
        Err(anyhow!("GET request to fetch the tarball failed"))
    }?;

    // verify the checksum
    let checksum = checksum.split('=').collect::<Vec<&str>>()[1];
    let mut hasher = sha2::Sha512::new();
    let mut file = std::fs::File::open(&file_path)?;
    std::io::copy(&mut file, &mut hasher)?;
    let hash_bytes = hasher.finalize();

    let decoded = <[u8; 64]>::from_hex(checksum).expect("");

    assert_eq!(hash_bytes[..], decoded);
    Ok(())
}