module IsomFolio.Core.Metadata.Xmp.PropReader

open System
open XmpCore

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