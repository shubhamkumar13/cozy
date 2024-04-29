[@@@warning "-32"]

let hello () = "Hello"

[@@@warning "-33-34-37"]

module Download = OpamDownload
module Url = OpamUrl
module Filename = OpamFilename
module Process = OpamProcess
module Repo = OpamRepository
module Types = OpamTypes
module Hash = OpamHash
open Rresult
open Rresult.R.Infix

let (let*) = (>>=)

type error = | NotAnArchive

let url = Url.of_string "https://erratique.ch/software/fmt/releases/fmt-0.9.0.tbz"
let hash = Hash.of_string "sha512=66cf4b8bb92232a091dfda5e94d1c178486a358cdc34b1eec516d48ea5acb6209c0dfcb416f0c516c50ddbddb3c94549a45e4a6d5c5fd1c81d3374dec823a83b"
let dir = Filename.Dir.of_string "/home/sk/ocaml-projects/test/download"

let untar () =
  let* job =
    Repo.pull_tree "fmt" dir [hash] [url] |> Result.ok in
  let* _ = Process.Job.run job |> Result.ok in
  Ok ()

let f () = 
    let overwrite = true in
    (* download from opam universe *)
    let* _ = Process.Job.run @@ Download.download ~overwrite url dir |> Result.ok in
    (* extract tarball *)
    let* _ = untar () in
    Result.ok ()

let () = Callback.register "hello" hello

let () = 
  let _ = f () |> Result.get_ok in
  print_endline "Hello, World!"