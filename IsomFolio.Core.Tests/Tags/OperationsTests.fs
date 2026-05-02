module IsomFolio.Tests.Tags.OperationsTests

open System
open System.IO
open Xunit
open IsomFolio.Tags.Operations

let private tempAssetPath () =
    let dir = Path.Combine(Path.GetTempPath(), $"isomfolio_tag_{Guid.NewGuid():N}")
    Directory.CreateDirectory(dir) |> ignore
    Path.Combine(dir, "image.jpg")

let private cleanup (assetPath: string) =
    try Directory.Delete(Path.GetDirectoryName(assetPath), true) with _ -> ()


module SidecarPath =

    [<Fact>]
    let ``replaces extension with .xmp`` () =
        Assert.Equal("/photos/img.xmp", sidecarPath "/photos/img.jpg")

    [<Fact>]
    let ``works on path with no extension`` () =
        Assert.Equal("/photos/img.xmp", sidecarPath "/photos/img")


module ReadTagsFromXmp =

    [<Fact>]
    let ``returns empty list when no sidecar exists`` () =
        async {
            let path = tempAssetPath ()
            try
                let! tags = readTagsFromXmp path
                Assert.Empty(tags)
            finally cleanup path
        } |> Async.RunSynchronously

    [<Fact>]
    let ``returns empty list for corrupt sidecar`` () =
        async {
            let path = tempAssetPath ()
            try
                File.WriteAllText(sidecarPath path, "not xml at all")
                let! tags = readTagsFromXmp path
                Assert.Empty(tags)
            finally cleanup path
        } |> Async.RunSynchronously


module WriteTagsToXmp =

    [<Fact>]
    let ``creates sidecar on first write`` () =
        async {
            let path = tempAssetPath ()
            try
                let! result = writeTagsToXmp path [ "vacation"; "beach" ]
                Assert.Equal(Ok (), result)
                Assert.True(File.Exists(sidecarPath path))
            finally cleanup path
        } |> Async.RunSynchronously

    [<Fact>]
    let ``round-trips tags through sidecar`` () =
        async {
            let path = tempAssetPath ()
            try
                let! _ = writeTagsToXmp path [ "alpha"; "beta"; "gamma" ]
                let! tags = readTagsFromXmp path
                Assert.Equal<string list>([ "alpha"; "beta"; "gamma" ], tags)
            finally cleanup path
        } |> Async.RunSynchronously

    [<Fact>]
    let ``overwrites existing tags`` () =
        async {
            let path = tempAssetPath ()
            try
                let! _ = writeTagsToXmp path [ "old" ]
                let! _ = writeTagsToXmp path [ "new1"; "new2" ]
                let! tags = readTagsFromXmp path
                Assert.Equal<string list>([ "new1"; "new2" ], tags)
            finally cleanup path
        } |> Async.RunSynchronously

    [<Fact>]
    let ``no .tmp file left on disk after write`` () =
        async {
            let path = tempAssetPath ()
            try
                let! _ = writeTagsToXmp path [ "tag" ]
                Assert.False(File.Exists(sidecarPath path + ".tmp"))
            finally cleanup path
        } |> Async.RunSynchronously


module AddTag =

    [<Fact>]
    let ``adds tag to empty sidecar`` () =
        async {
            let path = tempAssetPath ()
            try
                let! result = addTag path "vacation"
                Assert.Equal(Ok [ "vacation" ], result)
            finally cleanup path
        } |> Async.RunSynchronously

    [<Fact>]
    let ``is case-insensitive duplicate check`` () =
        async {
            let path = tempAssetPath ()
            try
                let! _ = addTag path "Vacation"
                let! result = addTag path "vacation"
                Assert.Equal(Ok [ "Vacation" ], result)
            finally cleanup path
        } |> Async.RunSynchronously

    [<Fact>]
    let ``appends to existing tags`` () =
        async {
            let path = tempAssetPath ()
            try
                let! _ = writeTagsToXmp path [ "existing" ]
                let! result = addTag path "new"
                Assert.Equal(Ok [ "existing"; "new" ], result)
            finally cleanup path
        } |> Async.RunSynchronously


module RemoveTag =

    [<Fact>]
    let ``removes matching tag`` () =
        async {
            let path = tempAssetPath ()
            try
                let! _ = writeTagsToXmp path [ "keep"; "remove" ]
                let! result = removeTag path "remove"
                Assert.Equal(Ok [ "keep" ], result)
            finally cleanup path
        } |> Async.RunSynchronously

    [<Fact>]
    let ``is case-insensitive`` () =
        async {
            let path = tempAssetPath ()
            try
                let! _ = writeTagsToXmp path [ "Vacation" ]
                let! result = removeTag path "vacation"
                Assert.Equal(Ok [], result)
            finally cleanup path
        } |> Async.RunSynchronously

    [<Fact>]
    let ``no-op when tag absent`` () =
        async {
            let path = tempAssetPath ()
            try
                let! _ = writeTagsToXmp path [ "keep" ]
                let! result = removeTag path "nothere"
                Assert.Equal(Ok [ "keep" ], result)
            finally cleanup path
        } |> Async.RunSynchronously
