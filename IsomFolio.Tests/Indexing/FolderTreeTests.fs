module IsomFolio.Tests.Indexing.FolderTreeTests

open System
open System.IO
open Xunit
open IsomFolio.Indexing.FolderTree

let private createTempFolderTree () =
    let root = Path.Combine(Path.GetTempPath(), $"isomfolio_tree_{Guid.NewGuid():N}")
    Directory.CreateDirectory(Path.Combine(root, "A", "B")) |> ignore
    Directory.CreateDirectory(Path.Combine(root, "C")) |> ignore
    root

module DisplayName =

    [<Fact>]
    let ``uses final path segment even with trailing separator`` () =
        let pathWithSeparator = Path.Combine("/tmp", "photos") + string Path.DirectorySeparatorChar
        Assert.Equal("photos", displayName pathWithSeparator)

module NormalizePath =

    [<Fact>]
    let ``resolves relative segments to a stable full path`` () =
        let baseDir = Path.Combine(Path.GetTempPath(), $"isomfolio_norm_{Guid.NewGuid():N}")
        let nested = Path.Combine(baseDir, "outer", "inner")
        Directory.CreateDirectory(nested) |> ignore
        try
            let rawPath = Path.Combine(baseDir, "outer", ".", "inner", "..", "inner")
            Assert.Equal(Path.GetFullPath(nested), normalizePath rawPath)
        finally
            Directory.Delete(baseDir, true)

module BuildForest =

    [<Fact>]
    let ``includes root folders and nested subdirectories`` () =
        let root = createTempFolderTree ()
        try
            let forest = buildForest [ root ]
            Assert.Single(forest) |> ignore

            let rootNode = forest.Head
            Assert.Equal(Path.GetFileName(root), rootNode.Name)
            Assert.Equal(2, rootNode.Children.Length)

            let childNames = rootNode.Children |> List.map (fun node -> node.Name)
            Assert.Equal<string list>([ "A"; "C" ], childNames)

            let nested = rootNode.Children |> List.find (fun node -> node.Name = "A")
            Assert.Single(nested.Children) |> ignore
            Assert.Equal("B", nested.Children.Head.Name)
        finally
            Directory.Delete(root, true)

    [<Fact>]
    let ``collapses nested roots under their ancestor`` () =
        let root = createTempFolderTree ()
        let inner = Path.Combine(root, "A")
        try
            let forest = buildForest [ inner; root ]
            Assert.Single(forest) |> ignore

            let rootNode = forest.Head
            Assert.Equal(normalizePath root, rootNode.Path)

            let childNames = rootNode.Children |> List.map (fun node -> node.Name)
            Assert.Equal<string list>([ "A"; "C" ], childNames)

            let innerNodesAtTopLevel =
                forest |> List.filter (fun node -> samePath node.Path inner)
            Assert.Empty(innerNodesAtTopLevel)
        finally
            Directory.Delete(root, true)
