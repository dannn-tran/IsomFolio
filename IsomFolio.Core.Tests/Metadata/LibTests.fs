module IsomFolio.Core.Tests.Metadata.LibTests

open System
open System.IO
open System.Text
open Xunit
open IsomFolio.Core.Metadata
open IsomFolio.Core.Metadata.Xmp

let private xmpPacketWithRating rating =
    $"""<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?><x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/"><xmp:Rating>{rating}</xmp:Rating></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="r"?>"""

let private stubJpeg = [| 0xFFuy; 0xD8uy; 0xFFuy; 0xD9uy |]

let private makeXmp rating =
    Some {
        Core       = { XmpCore.empty with Rating = Some rating }
        DublinCore = DublinCore.empty
    }

let private makeSources sidecar embedded =
    {
        Sidecar    = sidecar
        Embedded   = embedded
        Apple      = None
        FileSystem = { CreatedAt = DateTimeOffset.UtcNow; ModifiedAt = DateTimeOffset.UtcNow; SizeBytes = 4L }
    }

module OfSources =

    [<Fact>]
    let ``sidecar wins over embedded when both present`` () =
        let sources = makeSources (makeXmp 5) (makeXmp 3)
        let result = EmbeddedMetadata.ofSources sources
        Assert.Equal(Some 5, result.Xmp |> Option.bind (fun x -> x.Core.Rating))

    [<Fact>]
    let ``falls back to embedded when no sidecar`` () =
        let sources = makeSources None (makeXmp 3)
        let result = EmbeddedMetadata.ofSources sources
        Assert.Equal(Some 3, result.Xmp |> Option.bind (fun x -> x.Core.Rating))

    [<Fact>]
    let ``returns no XMP when both absent`` () =
        let sources = makeSources None None
        let result = EmbeddedMetadata.ofSources sources
        Assert.True(result.Xmp.IsNone)

    [<Fact>]
    let ``matches cached metadata when sidecar is the active source`` () =
        let xmp = makeXmp 4
        let sources = makeSources xmp (makeXmp 2)
        let cached = { Xmp = xmp; AppleMetadata = None }
        Assert.Equal(cached, EmbeddedMetadata.ofSources sources)


module ReadSources =

    [<Fact>]
    let ``reads sidecar rating and file system info`` () =
        let baseName = Guid.NewGuid().ToString("N")
        let imagePath = Path.Combine(Path.GetTempPath(), $"{baseName}.jpg")
        let sidecarPath = Path.Combine(Path.GetTempPath(), $"{baseName}.xmp")
        try
            File.WriteAllBytes(imagePath, stubJpeg)
            File.WriteAllText(sidecarPath, xmpPacketWithRating 5)
            let fi = FileInfo(imagePath)
            let sources = EmbeddedMetadata.readSources imagePath fi |> Async.RunSynchronously
            Assert.Equal(Some 5, sources.Sidecar |> Option.bind (fun x -> x.Core.Rating))
            Assert.Equal(None, sources.Embedded)
            Assert.Equal(int64 stubJpeg.Length, sources.FileSystem.SizeBytes)
        finally
            File.Delete(imagePath)
            if File.Exists(sidecarPath) then File.Delete(sidecarPath)
