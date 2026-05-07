module IsomFolio.Core.Tests.Metadata.TagTreeTests

open Xunit
open IsomFolio.Core.Metadata.TagTree

let private fullPaths (nodes: TagNode list) = flattenTree nodes |> List.sort


module BuildTree =

    [<Fact>]
    let ``flat list produces all Tagged roots`` () =
        let tree = buildTree [ "beach"; "travel"; "nature" ]
        Assert.Equal(3, tree.Length)
        Assert.True(tree |> List.forall (fun n -> n.Kind = Tagged))
        Assert.True(tree |> List.forall (fun n -> n.Children.IsEmpty))

    [<Fact>]
    let ``missing intermediate path becomes Ghost`` () =
        let tree = buildTree [ "person/John" ]
        Assert.Equal(1, tree.Length)
        let person = tree.[0]
        Assert.Equal("person", person.Segment)
        Assert.Equal(Ghost, person.Kind)
        Assert.Equal(1, person.Children.Length)
        Assert.Equal("John", person.Children.[0].Segment)
        Assert.Equal(Tagged, person.Children.[0].Kind)

    [<Fact>]
    let ``parent and child both present — parent is Tagged`` () =
        let tree = buildTree [ "person"; "person/John" ]
        Assert.Equal(1, tree.Length)
        let person = tree.[0]
        Assert.Equal(Tagged, person.Kind)
        Assert.Equal(1, person.Children.Length)
        Assert.Equal(Tagged, person.Children.[0].Kind)

    [<Fact>]
    let ``multiple children under same parent`` () =
        let tree = buildTree [ "person/Jane"; "person/John" ]
        let person = tree.[0]
        Assert.Equal(Ghost, person.Kind)
        Assert.Equal(2, person.Children.Length)

    [<Fact>]
    let ``depth 3 builds correctly`` () =
        let tree = buildTree [ "a/b/c" ]
        Assert.Equal(1, tree.Length)
        let a = tree.[0]
        Assert.Equal(Ghost, a.Kind)
        Assert.Equal("a", a.FullPath)
        let b = a.Children.[0]
        Assert.Equal(Ghost, b.Kind)
        Assert.Equal("a/b", b.FullPath)
        let c = b.Children.[0]
        Assert.Equal(Tagged, c.Kind)
        Assert.Equal("a/b/c", c.FullPath)


module FlattenTree =

    [<Fact>]
    let ``Ghost nodes excluded`` () =
        let tree = buildTree [ "person/John" ]
        Assert.Equal<string list>([ "person/John" ], flattenTree tree)

    [<Fact>]
    let ``round-trips Tagged-only inputs`` () =
        let tags = [ "beach"; "person"; "person/John"; "travel" ]
        Assert.Equal<string list>(tags |> List.sort, buildTree tags |> flattenTree |> List.sort)

    [<Fact>]
    let ``empty tree returns empty list`` () =
        Assert.Empty(flattenTree [])


module AddTag =

    [<Fact>]
    let ``adds leaf to empty tree`` () =
        let tree = addTag "beach" []
        Assert.Equal<string list>([ "beach" ], fullPaths tree)

    [<Fact>]
    let ``adds child under existing Ghost parent`` () =
        let tree = buildTree [ "person/John" ] |> addTag "person/Jane"
        Assert.Equal<string list>([ "person/Jane"; "person/John" ], fullPaths tree)

    [<Fact>]
    let ``creates Ghost ancestor for new deep tag`` () =
        let tree = addTag "a/b/c" []
        let a = tree.[0]
        Assert.Equal(Ghost, a.Kind)
        Assert.Equal("a/b/c", fullPaths tree |> List.head)

    [<Fact>]
    let ``no-op when tag already exists`` () =
        let tree = buildTree [ "beach" ]
        let tree2 = addTag "beach" tree
        Assert.Equal(1, tree2.Length)
        Assert.Equal(Tagged, tree2.[0].Kind)

    [<Fact>]
    let ``promotes Ghost to Tagged`` () =
        let tree = buildTree [ "person/John" ] |> addTag "person"
        let person = tree |> List.find (fun n -> n.Segment = "person")
        Assert.Equal(Tagged, person.Kind)
        Assert.Equal(1, person.Children.Length)


module RemoveTag =

    [<Fact>]
    let ``removes leaf node`` () =
        let tree = buildTree [ "beach"; "travel" ] |> removeTag "beach"
        Assert.Equal<string list>([ "travel" ], fullPaths tree)

    [<Fact>]
    let ``leaf removal cleans up empty Ghost parent`` () =
        let tree = buildTree [ "person/John" ] |> removeTag "person/John"
        Assert.Empty(tree)

    [<Fact>]
    let ``leaf removal leaves Ghost parent when other children remain`` () =
        let tree = buildTree [ "person/Jane"; "person/John" ] |> removeTag "person/John"
        Assert.Equal(1, tree.Length)
        Assert.Equal(Ghost, tree.[0].Kind)
        Assert.Equal(1, tree.[0].Children.Length)

    [<Fact>]
    let ``tagged parent with children demotes to Ghost`` () =
        let tree = buildTree [ "person"; "person/John" ] |> removeTag "person"
        let person = tree.[0]
        Assert.Equal(Ghost, person.Kind)
        Assert.Equal(1, person.Children.Length)
        Assert.Equal(Tagged, person.Children.[0].Kind)

    [<Fact>]
    let ``demotion then child removal cleans up Ghost`` () =
        let tree =
            buildTree [ "person"; "person/John" ]
            |> removeTag "person"
            |> removeTag "person/John"
        Assert.Empty(tree)


module RemoveSubtree =

    [<Fact>]
    let ``removes node and all descendants`` () =
        let tree = buildTree [ "person"; "person/Jane"; "person/John"; "travel" ]
        let result = removeSubtree "person" tree
        Assert.Equal<string list>([ "travel" ], fullPaths result)

    [<Fact>]
    let ``removes Ghost node and its descendants`` () =
        let tree = buildTree [ "a/b/c"; "a/b/d" ]
        let result = removeSubtree "a/b" tree
        Assert.Empty(result)

    [<Fact>]
    let ``cleans up Ghost ancestor after subtree removal`` () =
        let tree = buildTree [ "a/b/c" ]
        let result = removeSubtree "a/b" tree
        Assert.Empty(result)


module ReTag =

    [<Fact>]
    let ``Ghost node becomes Tagged`` () =
        let tree = buildTree [ "person/John" ]
        let result = reTag "person" tree
        let person = result.[0]
        Assert.Equal(Tagged, person.Kind)

    [<Fact>]
    let ``no-op when path not found`` () =
        let tree = buildTree [ "beach" ]
        let result = reTag "nobody" tree
        Assert.Equal<TagNode list>(tree, result)

    [<Fact>]
    let ``no-op when already Tagged`` () =
        let tree = buildTree [ "beach" ]
        let result = reTag "beach" tree
        Assert.Equal(Tagged, result.[0].Kind)
