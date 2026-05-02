module IsomFolio.Tags.Operations

open System
open System.IO
open System.Xml.Linq
open IsomFolio.Indexing

let private xmpNs = XNamespace.Get "adobe:ns:meta/"
let private rdfNs = XNamespace.Get "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
let private dcNs  = XNamespace.Get "http://purl.org/dc/elements/1.1/"

let sidecarPath (assetPath: string) : string =
    Path.ChangeExtension(assetPath, ".xmp")

let private emptyXmp () =
    XDocument(
        XProcessingInstruction("xpacket", "begin='' id='W5M0MpCehiHzreSzNTczkc9d'"),
        XElement(xmpNs + "xmpmeta",
            XAttribute(XNamespace.Xmlns + "x", xmpNs.NamespaceName),
            XElement(rdfNs + "RDF",
                XAttribute(XNamespace.Xmlns + "rdf", rdfNs.NamespaceName),
                XElement(rdfNs + "Description",
                    XAttribute(rdfNs + "about", ""),
                    XAttribute(XNamespace.Xmlns + "dc", dcNs.NamespaceName),
                    XElement(dcNs + "subject",
                        XElement(rdfNs + "Bag"))))),
        XProcessingInstruction("xpacket", "end='w'"))

let private parseTags (doc: XDocument) : string list =
    doc.Descendants(dcNs + "subject")
    |> Seq.tryHead
    |> Option.map (fun el ->
        el.Descendants(rdfNs + "li")
        |> Seq.map (fun li -> li.Value.Trim())
        |> Seq.filter (fun s -> s <> "")
        |> Seq.toList)
    |> Option.defaultValue []

let private setTags (doc: XDocument) (tags: string list) =
    let bag =
        doc.Descendants(dcNs + "subject")
        |> Seq.tryHead
        |> Option.bind (fun el -> el.Descendants(rdfNs + "Bag") |> Seq.tryHead)
    match bag with
    | None -> ()
    | Some b ->
        b.RemoveAll()
        for tag in tags do
            b.Add(XElement(rdfNs + "li", tag))

let readTagsFromXmp (assetPath: string) : Async<string list> =
    async {
        let xmp = sidecarPath assetPath
        if not (File.Exists(xmp)) then return []
        else
            try return parseTags (XDocument.Load(xmp))
            with _ -> return []
    }

let writeTagsToXmp (assetPath: string) (tags: string list) : Async<Result<unit, string>> =
    async {
        let xmp = sidecarPath assetPath
        let doc =
            if File.Exists(xmp) then
                try XDocument.Load(xmp)
                with _ -> emptyXmp ()
            else emptyXmp ()
        setTags doc tags
        let tmp = xmp + ".tmp"
        Watcher.registerSelfWrite xmp
        try
            doc.Save(tmp)
            File.Move(tmp, xmp, overwrite = true)
            return Ok ()
        with ex ->
            if File.Exists(tmp) then File.Delete(tmp)
            return Error ex.Message
    }

let addTag (assetPath: string) (tag: string) : Async<Result<string list, string>> =
    async {
        let! current = readTagsFromXmp assetPath
        let normalised = tag.Trim()
        if current |> List.exists (fun t -> String.Equals(t, normalised, StringComparison.OrdinalIgnoreCase)) then
            return Ok current
        else
            let updated = current @ [ normalised ]
            let! result = writeTagsToXmp assetPath updated
            match result with
            | Ok ()   -> return Ok updated
            | Error e -> return Error e
    }

let removeTag (assetPath: string) (tag: string) : Async<Result<string list, string>> =
    async {
        let! current = readTagsFromXmp assetPath
        let updated = current |> List.filter (fun t -> not (String.Equals(t, tag, StringComparison.OrdinalIgnoreCase)))
        if updated.Length = current.Length then return Ok current
        else
            let! result = writeTagsToXmp assetPath updated
            match result with
            | Ok ()   -> return Ok updated
            | Error e -> return Error e
    }
