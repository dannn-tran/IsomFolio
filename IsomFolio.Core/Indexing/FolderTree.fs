module IsomFolio.Indexing.FolderTree

open System
open System.IO
open IsomFolio.PathUtils

type FolderNode = {
    Name: string
    Path: string
    Children: FolderNode list
}

let displayName (path: string) =
    let name = Path.GetFileName(path)
    if String.IsNullOrWhiteSpace name then path else name

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
