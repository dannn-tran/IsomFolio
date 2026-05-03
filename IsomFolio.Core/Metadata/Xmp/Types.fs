module IsomFolio.Core.Metadata.Xmp.Types

open System

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
