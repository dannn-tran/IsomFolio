module IsomFolio.Core.Tests.Indexing.FolderTreeTests

open System
open System.IO
open Xunit
open IsomFolio.Core.PathUtils
open IsomFolio.Core.Indexing.FolderTree

let private createTempFolderTree () =
    let root = Path.Combine(Path.GetTempPath(), $"isomfolio_tree_{Guid.NewGuid():N}")
    Directory.CreateDirectory(Path.Combine(root, "A", "B")) |> ignore
    Directory.CreateDirectory(Path.Combine(root, "C")) |> ignore
    root

let private createTempDir name =
    let path = Path.Combine(Path.GetTempPath(), $"isomfolio_edge_{name}_{Guid.NewGuid():N}")
    Directory.CreateDirectory(path) |> ignore
    path

module DisplayName =
    [<Fact>]
    let ``uses final path segment even with trailing separator`` () =
        let pathWithSeparator = Path.Combine("/tmp", "photos") + string Path.DirectorySeparatorChar
        Assert.Equal("photos", displayName pathWithSeparator)

module NormalizePath =
    [<Fact>]
    let ``resolves relative segments to a stable full path`` () =
        let baseDir = createTempDir "norm"
        let nested = Path.Combine(baseDir, "outer", "inner")
        Directory.CreateDirectory(nested) |> ignore
        try
            let rawPath = Path.Combine(baseDir, "outer", ".", "inner", "..", "inner")
            let expected = normalizePath nested
            Assert.Equal(expected, normalizePath rawPath)
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
            Assert.Equal<string list>([ "a"; "c" ], childNames)

            let nested = rootNode.Children |> List.find (fun node -> node.Name = "a")
            Assert.Single(nested.Children) |> ignore
            Assert.Equal("b", nested.Children.Head.Name)
        finally
            Directory.Delete(root, true)

    [<Fact>]
    let ``collapses nested roots under their ancestor`` () =
        let root = createTempFolderTree ()
        let inner = Path.Combine(root, "A")
        try
            // Test redundancy: /root and /root/A should collapse to /root
            let forest = buildForest [ inner; root ]
            Assert.Single(forest) |> ignore

            let rootNode = forest.Head
            Assert.Equal(normalizePath root, rootNode.Path)

            let childNames = rootNode.Children |> List.map (fun node -> node.Name)
            Assert.Equal<string list>([ "a"; "c" ], childNames)

            let innerNodesAtTopLevel =
                forest |> List.filter (fun node -> samePath node.Path inner)
            Assert.Empty(innerNodesAtTopLevel)
        finally
            Directory.Delete(root, true)

    [<Fact>]
    let ``groups siblings under their common nearest ancestor without unrelated siblings`` () =
        let root = createTempFolderTree ()
        // We add "a" and "c" explicitly, "D_unrelated" should be ignored
        let sibling1 = Path.Combine(root, "a")
        let sibling2 = Path.Combine(root, "c")
        let unrelated = Path.Combine(root, "D_unrelated")
        Directory.CreateDirectory(unrelated) |> ignore
        
        try
            let forest = buildForest [ sibling1; sibling2 ]
            Assert.Single(forest) |> ignore
            
            let rootNode = forest.Head
            Assert.Equal(normalizePath root, rootNode.Path)
            
            let childNames = rootNode.Children |> List.map (fun n -> n.Name)
            Assert.Equal<string list>([ "a"; "c" ], childNames)
        finally
            Directory.Delete(root, true)

    [<Fact>]
    let ``non-existent paths are ignored or handled gracefully`` () =
        let fakePath = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"))
        let forest = buildForest [ fakePath ]
        Assert.Single(forest) |> ignore
        Assert.Empty(forest.Head.Children)

    [<Fact>]
    let ``deeply nested structures don't overflow stack`` () =
        let root = createTempDir "deep"
        let mutable current = root
        for i in 1 .. 50 do
            current <- Path.Combine(current, $"level_{i}")
            Directory.CreateDirectory(current) |> ignore
        
        try
            let forest = buildForest [ root ]
            Assert.Single(forest) |> ignore
        finally
            Directory.Delete(root, true)

    [<Fact>]
    let ``unrelated roots remain separate in forest`` () =
        let root1 = createTempDir "unrelated1"
        let root2 = createTempDir "unrelated2"
        try
            let forest = buildForest [ root1; root2 ]
            let common = Path.GetDirectoryName(root1)
            if String.Equals(common, Path.GetDirectoryName(root2)) then
                Assert.Single(forest) |> ignore
            else
                Assert.Equal(2, forest.Length)
        finally
            Directory.Delete(root1, true)
            Directory.Delete(root2, true)
