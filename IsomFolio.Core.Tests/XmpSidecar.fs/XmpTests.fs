module IsomFolio.Tests.Tags.XmpTests

open System
open System.IO
open Xunit
open IsomFolio.Core.Metadata.Xmp

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


module ReadTagsFromSidecar =

    [<Fact>]
    let ``returns error when no sidecar exists`` () =
        let path = tempAssetPath ()
        try
            let result = readTagsFromSidecar path
            Assert.Equal(Error (FileNotFound path), result)
        finally cleanup path

    [<Fact>]
    let ``returns error for corrupt sidecar`` () =
        let path = tempAssetPath ()
        try
            File.WriteAllText(sidecarPath path, "not xml at all")
            let result = readTagsFromSidecar path
            Assert.True(result.IsError)
        finally cleanup path


// module WriteTagsToXmp =
//
//     [<Fact>]
//     let ``creates sidecar on first write`` () =
//         async {
//             let path = tempAssetPath ()
//             try
//                 let! result = writeTagsToXmp path [ "vacation"; "beach" ]
//                 Assert.Equal(Ok (), result)
//                 Assert.True(File.Exists(sidecarPath path))
//             finally cleanup path
//         } |> Async.RunSynchronously
//
//     [<Fact>]
//     let ``round-trips tags through sidecar`` () =
//         async {
//             let path = tempAssetPath ()
//             try
//                 let! _ = writeTagsToXmp path [ "alpha"; "beta"; "gamma" ]
//                 let! result = readTagsFromXmpSidecar path
//                 Assert.Equal(Ok [ "alpha"; "beta"; "gamma" ], result)
//             finally cleanup path
//         } |> Async.RunSynchronously
//
//     [<Fact>]
//     let ``overwrites existing tags`` () =
//         async {
//             let path = tempAssetPath ()
//             try
//                 let! _ = writeTagsToXmp path [ "old" ]
//                 let! _ = writeTagsToXmp path [ "new1"; "new2" ]
//                 let! result = readTagsFromXmpSidecar path
//                 Assert.Equal(Ok [ "new1"; "new2" ], result)
//             finally cleanup path
//         } |> Async.RunSynchronously
//
//     [<Fact>]
//     let ``no .tmp file left on disk after write`` () =
//         async {
//             let path = tempAssetPath ()
//             try
//                 let! _ = writeTagsToXmp path [ "tag" ]
//                 Assert.False(File.Exists(sidecarPath path + ".tmp"))
//             finally cleanup path
//         } |> Async.RunSynchronously
