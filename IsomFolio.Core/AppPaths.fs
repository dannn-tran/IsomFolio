module IsomFolio.AppPaths

open System
open System.IO

let private appDataRoot () =
    Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "IsomFolio")

let mutable private catalogRoot: string option = None

let private requireCatalogRoot () =
    match catalogRoot with
    | Some p -> p
    | None   -> failwith "No catalog open. Call setCatalogRoot or createCatalog first."

let dbPath ()            = Path.Combine(requireCatalogRoot (), "catalog.db")
let thumbnailCacheDir () = Path.Combine(requireCatalogRoot (), "thumbnails")
let sessionFilePath ()   = Path.Combine(appDataRoot (), "session.json")

let ensureDirectories () =
    Directory.CreateDirectory(thumbnailCacheDir ()) |> ignore

let setCatalogRoot (path: string) =
    catalogRoot <- Some path
    Directory.CreateDirectory(Path.Combine(path, "thumbnails")) |> ignore

let createCatalog (parentDir: string) (name: string) : string =
    let catalogPath = Path.Combine(parentDir, name + ".isomfolio")
    Directory.CreateDirectory(Path.Combine(catalogPath, "thumbnails")) |> ignore
    catalogPath

let readLastCatalog () : string option =
    let f = sessionFilePath ()
    if File.Exists f then
        try Some(File.ReadAllText(f).Trim())
        with _ -> None
    else None

let saveLastCatalog (path: string) =
    Directory.CreateDirectory(appDataRoot ()) |> ignore
    File.WriteAllText(sessionFilePath (), path)
