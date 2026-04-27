module IsomFolio.Indexing.FolderTree

open System
open System.IO

type FolderNode = {
    Name: string
    Path: string
    Children: FolderNode list
}

let private trimTrailingSeparators (path: string) =
    if String.IsNullOrWhiteSpace path then path
    else path.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar)

let normalizePath (path: string) =
    path
    |> Path.GetFullPath
    |> trimTrailingSeparators

let displayName (path: string) =
    let normalized = normalizePath path
    let name = Path.GetFileName(normalized)
    if String.IsNullOrWhiteSpace name then normalized else name

let private pathComparison =
    if OperatingSystem.IsWindows() then StringComparison.OrdinalIgnoreCase
    else StringComparison.Ordinal

let samePath (left: string) (right: string) =
    String.Equals(left, right, pathComparison)

let private isDescendantPath (ancestor: string) (candidate: string) =
    if samePath ancestor candidate then
        false
    else
        let prefix = ancestor + string Path.DirectorySeparatorChar
        candidate.StartsWith(prefix, pathComparison)

let rec private buildNode (path: string) =
    let children =
        try
            Directory.EnumerateDirectories(path)
            |> Seq.sortBy displayName
            |> Seq.map buildNode
            |> Seq.toList
        with _ ->
            []

    {
        Name = displayName path
        Path = path
        Children = children
    }

let buildForest (rootFolders: string list) =
    rootFolders
    |> List.map normalizePath
    |> List.distinct
    |> List.sortBy _.Length
    |> List.fold (fun acc path ->
        if acc |> List.exists (fun existing -> isDescendantPath existing path || samePath existing path) then
            acc
        else
            acc @ [ path ]) []
    |> List.map buildNode
