module IsomFolio.Core.Tags.Operations

open System
open System.IO
open XmpCore
open XmpCore.Options
open IsomFolio.Core.Indexing

type ExtractionError =
    | XmpSidecarAbsent
    | XmpParseFailed of string
    | DcSubjectAbsent


let sidecarPath (assetPath: string) : string =
    Path.ChangeExtension(assetPath, ".xmp")

[<Literal>]
let private subject = "subject"

let private parseTags (xmp: IXmpMeta) : Result<string list, ExtractionError> =
    if xmp.DoesPropertyExist(XmpConstants.NsDC, subject) then
        let count = xmp.CountArrayItems(XmpConstants.NsDC, subject)
        Ok [ for i in 1 .. count do
                let item = xmp.GetArrayItem(XmpConstants.NsDC, subject, i)
                if not (isNull item) && not (isNull item.Value) then
                    yield item.Value ]
    else Error DcSubjectAbsent

let private setTags (xmp: IXmpMeta) (tags: string list) =
    failwith "Not implemented yet"
    // xmp.DeleteProperty(XmpConstants.NsDC, subject)
    // if not (List.isEmpty tags) then
    //     // dc:subject should be a Bag (unordered array)
    //     let arrayOptions = PropertyOptions().SetArray(true)
    //     for tag in tags do
    //         xmp.AppendArrayItem(XmpConstants.NsDC, subject, arrayOptions, tag, null)

let readTagsFromXmpSidecar (assetPath: string) : Async<Result<string list, ExtractionError>> =
    async {
        let xmpPath = sidecarPath assetPath
        if not (File.Exists(xmpPath)) then
            return Error XmpSidecarAbsent
        else
            use stream = File.OpenRead(xmpPath)
            let xmp =
                try
                    Ok (XmpMetaFactory.Parse(stream))
                with
                | e -> Error (XmpParseFailed(e.ToString()))
            return xmp |> Result.bind parseTags
    }

let writeTagsToXmp (assetPath: string) (tags: string list) : Async<Result<unit, string>> =
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
    //     setTags xmp tags
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

let addTag (assetPath: string) (tag: string) : Async<Result<string list, string>> =
    failwith "Not implemented yet"
    // async {
    //     let! current = readTagsFromXmp assetPath
    //     let normalised = tag.Trim()
    //     if current |> List.exists (fun t -> String.Equals(t, normalised, StringComparison.OrdinalIgnoreCase)) then
    //         return Ok current
    //     else
    //         let updated = current @ [ normalised ]
    //         let! result = writeTagsToXmp assetPath updated
    //         match result with
    //         | Ok ()   -> return Ok updated
    //         | Error e -> return Error e
    // }

let removeTag (assetPath: string) (tag: string) : Async<Result<string list, string>> =
    failwith "Not implemented yet"
    // async {
    //     let! current = readTagsFromXmp assetPath
    //     let updated = current |> List.filter (fun t -> not (String.Equals(t, tag, StringComparison.OrdinalIgnoreCase)))
    //     if updated.Length = current.Length then return Ok current
    //     else
    //         let! result = writeTagsToXmp assetPath updated
    //         match result with
    //         | Ok ()   -> return Ok updated
    //         | Error e -> return Error e
    // }
