module IsomFolio.Core.Metadata.Xmp

open System.IO
open MetadataExtractor
open MetadataExtractor.Formats.Xmp
open XmpCore

type ExtractionError =
    | FileNotFound of string
    | FileParseFailed of exn


let sidecarPath (assetPath: string) : string =
    Path.ChangeExtension(assetPath, ".xmp")

[<Literal>]
let private subject = "subject"

let private parseTags (xmp: IXmpMeta) : string list =
    if xmp.DoesPropertyExist(XmpConstants.NsDC, subject) then
        let count = xmp.CountArrayItems(XmpConstants.NsDC, subject)
        [ 1 .. count ]
        |> List.map (fun i ->
            xmp.GetArrayItem(XmpConstants.NsDC, "subject", i).Value)
    else []

let readTagsFromSidecar (assetPath: string) : Result<string list, ExtractionError> =
    let xmpPath = sidecarPath assetPath
    if not (File.Exists(xmpPath)) then
        Error (FileNotFound xmpPath)
    else
        use stream = File.OpenRead(xmpPath)
        let xmp =
            try
                Ok (XmpMetaFactory.Parse(stream))
            with
            | e -> Error (FileParseFailed e)
        xmp |> Result.map parseTags

let readEmbeddedTags (assetPath: string): Result<string list, ExtractionError> =
    if not (File.Exists(assetPath)) then
        Error (FileNotFound assetPath)
    else
        use stream = File.OpenRead assetPath
        let dirs =
            try
                Ok (ImageMetadataReader.ReadMetadata stream)
            with
            | e -> Error (FileParseFailed e)

        dirs |> Result.map (fun d ->
            d
            |> Seq.tryPick (function
                | :? XmpDirectory as xmpDir -> Some xmpDir
                | _ -> None)
            |> Option.map (fun xmpDir -> xmpDir.XmpMeta |> parseTags)
            |> Option.defaultValue [])

let writeTags (assetPath: string) (tags: string list) : Async<Result<unit, string>> =
    failwith "Not implemented yet"
    // async {
    //     let xmpPath = sidecarPath assetPath
    //     let xmp =
    //         if File.Exists(xmpPath) then
    //             try
    //                 use stream = File.OpenRead(xmpPath)
    //                 XmpMetaFactory.Parse(stream)
    //             with _ -> XmpMetaFactory.Create()
    //         else XmpMetaFactory.Create()
    //     
        // xmp.DeleteProperty(XmpConstants.NsDC, subject)
        // if not (List.isEmpty tags) then
        //     // dc:subject should be a Bag (unordered array)
        //     let arrayOptions = PropertyOptions().SetArray(true)
        //     for tag in tags do
        //         xmp.AppendArrayItem(XmpConstants.NsDC, subject, arrayOptions, tag, null)
    //     
    //     let tmp = xmpPath + ".tmp"
    //     Watcher.registerSelfWrite xmpPath
    //     try
    //         do
    //             use stream = File.Create(tmp)
    //             let options = SerializeOptions().SetUsePacket(true)
    //             XmpMetaFactory.Serialize(xmp, stream, options)
    //         File.Move(tmp, xmpPath, overwrite = true)
    //         return Ok ()
    //     with ex ->
    //         if File.Exists(tmp) then try File.Delete(tmp) with _ -> ()
    //         return Error ex.Message
    // }
