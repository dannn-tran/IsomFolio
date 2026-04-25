module IsomFolio.AppPaths

open System
open System.IO

/// Root config/data dir: %APPDATA%\IsomFolio (Win), ~/Library/Application Support/IsomFolio (macOS), ~/.local/share/IsomFolio (Linux)
let appDataRoot () =
    Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "IsomFolio")

let thumbnailCacheDir () = Path.Combine(appDataRoot (), "cache", "thumbnails")

let dbPath () = Path.Combine(appDataRoot (), "isomfolio.db")

let sessionFilePath () = Path.Combine(appDataRoot (), "session.json")

/// Creates all required app directories if they don't exist
let ensureDirectories () =
    Directory.CreateDirectory(thumbnailCacheDir ()) |> ignore
