module IsomFolio.Core.Tests.Metadata.Xmp.LibTests

open System
open System.IO
open System.Text
open Xunit
open IsomFolio.Core.Metadata
open IsomFolio.Core.Metadata.Xmp

let private xmpPacketWithRating rating =
    $"""<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?><x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/"><xmp:Rating>{rating}</xmp:Rating></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="r"?>"""

let private makeJpegWithXmp (xmpPacket: string) =
    let identifier = [| yield! Encoding.ASCII.GetBytes("http://ns.adobe.com/xap/1.0/"); yield 0uy |]
    let xmpBytes = Encoding.UTF8.GetBytes(xmpPacket)
    let payload = Array.append identifier xmpBytes
    let app1Length = payload.Length + 2
    use ms = new MemoryStream()
    ms.WriteByte(0xFFuy); ms.WriteByte(0xD8uy)
    ms.WriteByte(0xFFuy); ms.WriteByte(0xE1uy)
    ms.WriteByte(byte (app1Length >>> 8))
    ms.WriteByte(byte (app1Length &&& 0xFF))
    ms.Write(payload)
    ms.WriteByte(0xFFuy); ms.WriteByte(0xD9uy)
    ms.ToArray()

let private stubJpeg = [| 0xFFuy; 0xD8uy; 0xFFuy; 0xD9uy |]

module GetEmbedded =

    [<Fact>]
    let ``returns None for file without XMP`` () =
        let path = Path.Combine(Path.GetTempPath(), $"{Guid.NewGuid():N}.jpg")
        try
            File.WriteAllBytes(path, stubJpeg)
            match XmpMetadata.getEmbedded path with
            | Ok None -> ()
            | other -> Assert.Fail($"Expected Ok None, got {other}")
        finally
            File.Delete(path)

    [<Fact>]
    let ``returns metadata for file with embedded XMP`` () =
        let path = Path.Combine(Path.GetTempPath(), $"{Guid.NewGuid():N}.jpg")
        try
            File.WriteAllBytes(path, makeJpegWithXmp (xmpPacketWithRating 4))
            match XmpMetadata.getEmbedded path with
            | Ok (Some xmp) -> Assert.Equal(Some 4, xmp.Core.Rating)
            | other -> Assert.Fail($"Expected Ok (Some ...), got {other}")
        finally
            File.Delete(path)


module FileMetadataRead =

    [<Fact>]
    let ``sidecar wins over embedded — embedded probe skipped when sidecar present`` () =
        let baseName = Guid.NewGuid().ToString("N")
        let imagePath = Path.Combine(Path.GetTempPath(), $"{baseName}.jpg")
        let sidecarPath = Path.Combine(Path.GetTempPath(), $"{baseName}.xmp")
        try
            File.WriteAllBytes(imagePath, stubJpeg)
            File.WriteAllText(sidecarPath, xmpPacketWithRating 5)
            let meta = FileMetadata.read imagePath |> Async.RunSynchronously
            match meta.Xmp with
            | Some xmp -> Assert.Equal(Some 5, xmp.Core.Rating)
            | None -> Assert.Fail("Expected sidecar XMP")
        finally
            File.Delete(imagePath)
            if File.Exists(sidecarPath) then File.Delete(sidecarPath)
