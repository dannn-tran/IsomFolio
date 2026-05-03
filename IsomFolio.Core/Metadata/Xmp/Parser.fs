module IsomFolio.Core.Metadata.Xmp.Parser

open System
open System.IO
open IsomFolio.Core.Metadata.Xmp.Types
open MetadataExtractor
open MetadataExtractor.Formats.Xmp
open XmpCore


module private XmpHelper =
    let private toDateTimeOffset (xdt: IXmpDateTime) : DateTimeOffset =
        let offset =
            if xdt.HasTimeZone then xdt.TimeZone.BaseUtcOffset
            else TimeSpan.Zero
        DateTimeOffset(xdt.Year, xdt.Month, xdt.Day,
                       xdt.Hour, xdt.Minute, xdt.Second,
                       offset)

    let getDateTimeOffset ns path (xmp: IXmpMeta)  =
        try
            xmp.GetPropertyDate(ns, path) |> Option.ofObj
        with _ -> None
        |> Option.map toDateTimeOffset
        
    let getString ns path (xmp: IXmpMeta) =
        try
            xmp.GetPropertyString(ns, path) |> Option.ofObj
        with _ -> None
        
    let getInt ns path (xmp: IXmpMeta) =
        try
            xmp.GetPropertyInteger(ns, path) |> Some
        with _ -> None

    let getAltTextDefault ns path (xmp: IXmpMeta) =
        try
            xmp.GetLocalizedText(ns, path, null, "x-default") |> Option.ofObj
        with _ -> None
        |> Option.map _.Value
        
    let getSeq ns path (xmp: IXmpMeta) =
        try
            let count = xmp.CountArrayItems(ns, path)
            [ 1..count ]
            |> Seq.map (fun i -> xmp.GetArrayItem(ns, path, i))
        with _ -> []
    
    let getStringList ns path (xmp: IXmpMeta) =
        xmp
        |> getSeq ns path
        |> Seq.map _.Value
        |> Seq.filter (fun s -> not (String.IsNullOrEmpty(s)))
        |> Seq.toList

let private parseCore (xmp: IXmpMeta) : XmpCore =
    {
        CreateDate   = xmp |> XmpHelper.getDateTimeOffset XmpConstants.NsXmp "CreateDate"
        ModifyDate   = xmp |> XmpHelper.getDateTimeOffset XmpConstants.NsXmp "ModifyDate"
        MetadataDate = xmp |> XmpHelper.getDateTimeOffset XmpConstants.NsXmp "MetadataDate"
        CreatorTool  = xmp |> XmpHelper.getString XmpConstants.NsXmp "CreatorTool"
        Rating       = xmp |> XmpHelper.getInt XmpConstants.NsXmp "Rating"
        Label        = xmp |> XmpHelper.getString XmpConstants.NsXmp "Label"
    }

let private parseDublinCore (xmp: IXmpMeta) : DublinCore =
    {
        Title       = xmp |> XmpHelper.getAltTextDefault XmpConstants.NsDC "title"
        Description = xmp |> XmpHelper.getAltTextDefault XmpConstants.NsDC "description"
        Creator     = xmp |> XmpHelper.getStringList XmpConstants.NsDC "creator"
        Rights      = xmp |> XmpHelper.getAltTextDefault XmpConstants.NsDC "rights"
        Subject     = xmp |> XmpHelper.getStringList XmpConstants.NsDC "subject"
        Format      = xmp |> XmpHelper.getString XmpConstants.NsDC "format"
    }
    
let parseMeta (xmp: IXmpMeta) : XmpMetadata =
    {
        Core = xmp |> parseCore
        DublinCore = xmp |> parseDublinCore
    }

let getXmpEmbedded (assetPath: string): Result<XmpMetadata option, exn> =
    if not (File.Exists(assetPath)) then
        Error (FileNotFoundException assetPath)
    else
        use stream = File.OpenRead assetPath
        try
            Ok (ImageMetadataReader.ReadMetadata stream)
        with e -> Error e
        |> Result.map (fun d ->
            d
            |> Seq.tryPick (function
                | :? XmpDirectory as xmpDir -> Some xmpDir
                | _ -> None)
            |> Option.map (fun xmpDir -> xmpDir.XmpMeta |> parseMeta))

let sidecarPath (assetPath: string) : string =
    Path.ChangeExtension(assetPath, ".xmp")

let getXmpSidecar (assetPath: string): Result<XmpMetadata option, exn> =
    let xmpPath = sidecarPath assetPath
    if not (File.Exists(xmpPath)) then
        Ok None
    else
        use stream = File.OpenRead(xmpPath)
        try
            Ok (XmpMetaFactory.Parse(stream))
        with e -> Error e
        |> Result.map (fun meta -> meta |> parseMeta |> Some)

