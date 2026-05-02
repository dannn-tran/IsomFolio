module IsomFolio.Core.AppPaths

open System
open System.IO

let private appDataRoot () =
    Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "IsomFolio")

let dbPath (catalogDir: string)            = Path.Combine(catalogDir, "catalog.db")
let thumbnailCacheDir (catalogDir: string) = Path.Combine(catalogDir, "thumbnails")
let private sessionFilePath () = Path.Combine(appDataRoot (), "session.json")

let ensureDirectories (catalogDir: string) =
    Directory.CreateDirectory(thumbnailCacheDir catalogDir) |> ignore

let createCatalog (parentDir: string) (name: string) : string =
    let catalogPath = Path.Combine(parentDir, name + ".isomfolio")
    Directory.CreateDirectory(Path.Combine(catalogPath, "thumbnails")) |> ignore
    catalogPath

type Session = { CatalogPath: string; Folders: string list }

let readLastSession () : Session option =
    let f = sessionFilePath ()
    if not (File.Exists f) then None
    else
        try
            use doc = System.Text.Json.JsonDocument.Parse(File.ReadAllText f)
            let root = doc.RootElement
            let catalogPath = root.GetProperty("catalogPath").GetString()
            let folders =
                let arr = root.GetProperty("folders")
                [ for i in 0 .. arr.GetArrayLength() - 1 -> arr.[i].GetString() ]
            Some { CatalogPath = catalogPath; Folders = folders }
        with _ -> None

let saveSession (s: Session) =
    Directory.CreateDirectory(appDataRoot ()) |> ignore
    let data = {| catalogPath = s.CatalogPath; folders = s.Folders |}
    File.WriteAllText(sessionFilePath (), System.Text.Json.JsonSerializer.Serialize(data))
