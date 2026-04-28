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
    let normalized = normalizePath path
    let name = Path.GetFileName(normalized)
    if String.IsNullOrWhiteSpace name then normalized else name

let private findCommonPath (paths: string list) =
    match paths with
    | [] -> None
    | [ p ] -> Some p
    | head :: tail ->
        let commonPrefix (a: string) (b: string) =
            let segsA = a.Split(Path.DirectorySeparatorChar, StringSplitOptions.RemoveEmptyEntries)
            let segsB = b.Split(Path.DirectorySeparatorChar, StringSplitOptions.RemoveEmptyEntries)
            let driveA = Path.GetPathRoot(a)
            let driveB = Path.GetPathRoot(b)
            if not (String.Equals(driveA, driveB, StringComparison.Ordinal)) then
                None
            else
                let matchingSegs =
                    (segsA, segsB)
                    ||> Seq.zip
                    |> Seq.takeWhile (fun (s1, s2) -> String.Equals(s1, s2, StringComparison.Ordinal))
                    |> Seq.map fst
                    |> Seq.toList
                
                if List.isEmpty matchingSegs then
                    if String.IsNullOrEmpty driveA then None else Some driveA
                else
                    let combined = String.concat (string Path.DirectorySeparatorChar) matchingSegs
                    if a.StartsWith(string Path.DirectorySeparatorChar) then
                        Some (string Path.DirectorySeparatorChar + combined)
                    else
                        Some combined

        tail |> List.fold (fun acc p ->
            match acc with
            | None -> None
            | Some current -> commonPrefix current p
        ) (Some head)

let rec private buildNodeFiltered (path: string) (allowedRoots: string list) =
    let isPathRelevant (p: string) =
        allowedRoots |> List.exists (fun root ->
            samePath p root || isDescendantPath root p || isDescendantPath p root
        )

    let children =
        try
            Directory.EnumerateDirectories(path)
            |> Seq.map normalizePath
            |> Seq.filter isPathRelevant
            |> Seq.sortBy displayName
            |> Seq.map (fun p -> buildNodeFiltered p allowedRoots)
            |> Seq.toList
        with _ ->
            []

    {
        Name = displayName path
        Path = path
        Children = children
    }

/// Given a set of root paths, build a forest of trees.
/// If roots overlap (e.g. /A and /A/B), the most ancestral root (/A) wins.
/// If siblings are provided, they are grouped under their common nearest ancestor,
/// but only the branches leading to requested roots are shown.
let buildForest (rootFolders: string list) =
    let normalized =
        rootFolders
        |> List.map normalizePath
        |> List.distinct
        |> List.sortBy _.Length
        |> List.fold (fun acc path ->
            if acc |> List.exists (fun existing -> isDescendantPath existing path || samePath existing path) then
                acc
            else
                acc @ [ path ]) []
    
    match normalized with
    | [] -> []
    | [ single ] -> [ buildNodeFiltered single normalized ]
    | multiple ->
        match findCommonPath multiple with
        | Some common when not (List.contains common multiple) ->
            // We have a common parent that isn't one of the roots.
            // Build the node for the common parent, but ONLY include branches
            // that lead to our requested roots.
            [ buildNodeFiltered common multiple ]
        | _ ->
            multiple |> List.map (fun p -> buildNodeFiltered p [p])
