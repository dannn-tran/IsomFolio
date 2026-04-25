module IsomFolio.AppPaths

open System
open System.IO

let private appDataRoot () =
    Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "IsomFolio")

let dbPath (catalogDir: string)            = Path.Combine(catalogDir, "catalog.db")
let thumbnailCacheDir (catalogDir: string) = Path.Combine(catalogDir, "thumbnails")
let sessionFilePath ()                     = Path.Combine(appDataRoot (), "session.json")

let ensureDirectories (catalogDir: string) =
    Directory.CreateDirectory(thumbnailCacheDir catalogDir) |> ignore

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
