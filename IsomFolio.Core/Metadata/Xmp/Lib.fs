namespace IsomFolio.Core.Metadata.Xmp

open System
open System.IO
open MetadataExtractor
open MetadataExtractor.Formats.Xmp
open XmpCore

/// xmp: — core XMP properties
type XmpCore = {
    CreateDate    : DateTimeOffset option
    ModifyDate    : DateTimeOffset option
    MetadataDate  : DateTimeOffset option
    CreatorTool   : string option
    Rating        : int option          // 0–5
    Label         : string option       // e.g. "Red", "Green"
}

module XmpCore =
    let empty = {
        CreateDate = None; ModifyDate = None; MetadataDate = None
        CreatorTool = None; Rating = None; Label = None
    }
    
    let fromXmp (xmp: IXmpMeta) : XmpCore =
        {
            CreateDate   = xmp |> PropReader.getDateTimeOffset XmpConstants.NsXmp "CreateDate"
            ModifyDate   = xmp |> PropReader.getDateTimeOffset XmpConstants.NsXmp "ModifyDate"
            MetadataDate = xmp |> PropReader.getDateTimeOffset XmpConstants.NsXmp "MetadataDate"
            CreatorTool  = xmp |> PropReader.getString XmpConstants.NsXmp "CreatorTool"
            Rating       = xmp |> PropReader.getInt XmpConstants.NsXmp "Rating"
            Label        = xmp |> PropReader.getString XmpConstants.NsXmp "Label"
        }

/// dc: — Dublin Core
type DublinCore = {
    Title       : string option
    Description : string option
    Creator     : string list           // author(s)
    Rights      : string option
    Subject     : string list           // keywords / tags
    Format      : string option         // MIME type
}

module DublinCore =
    let empty = {
        Title = None; Description = None; Creator = []
        Rights = None; Subject = []; Format = None
    }
    
    let fromXmp (xmp: IXmpMeta) : DublinCore =
        {
            Title       = xmp |> PropReader.getAltTextDefault XmpConstants.NsDC "title"
            Description = xmp |> PropReader.getAltTextDefault XmpConstants.NsDC "description"
            Creator     = xmp |> PropReader.getStringList XmpConstants.NsDC "creator"
            Rights      = xmp |> PropReader.getAltTextDefault XmpConstants.NsDC "rights"
            Subject     = xmp |> PropReader.getStringList XmpConstants.NsDC "subject"
            Format      = xmp |> PropReader.getString XmpConstants.NsDC "format"
        }

/// Top-level envelope
type XmpMetadata = {
    Core       : XmpCore
    DublinCore : DublinCore
}

module XmpMetadata =
    let empty = {
        Core       = XmpCore.empty
        DublinCore = DublinCore.empty
    }
    
    let fromXmp (xmp: IXmpMeta) : XmpMetadata =
        {
            Core       = xmp |> XmpCore.fromXmp
            DublinCore = xmp |> DublinCore.fromXmp
        }

    let parseEmbedded (assetPath: string): Result<XmpMetadata option, exn> =
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
                |> Option.map (fun xmpDir -> xmpDir.XmpMeta |> fromXmp))

    let parseSidecar (assetPath: string): Result<XmpMetadata option, exn> =
        let xmpPath = Path.ChangeExtension(assetPath, ".xmp")
        if not (File.Exists(xmpPath)) then
            Ok None
        else
            use stream = File.OpenRead(xmpPath)
            try
                Ok (XmpMetaFactory.Parse(stream))
            with e -> Error e
            |> Result.map (fun meta -> meta |> fromXmp |> Some)
