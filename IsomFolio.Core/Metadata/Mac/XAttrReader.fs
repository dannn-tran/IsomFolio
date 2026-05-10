module IsomFolio.Core.Metadata.Mac.XAttrReader

open System
open System.Runtime.InteropServices
open System.Runtime.Versioning

[<DllImport("libc", SetLastError = true)>]
extern int getxattr(string path, string name, byte[] value, unativeint size, uint32 position, int options)

[<Literal>]
let private ENOATTR = 93  // implies extended attribute does not exist on this file

[<SupportedOSPlatform("macos")>]
let private readBytes attrName (filePath: string) : Result<byte[] option, string> =
    // Probe for the byte length of the xattr value.
    let size = getxattr(filePath, attrName, null, 0un, 0u, 0)
    if size < 0 then
        match Marshal.GetLastWin32Error() with
        | e when e = ENOATTR -> Ok None
        | e                  -> Error $"getxattr size probe errno %d{e}"
    else
        // Over-allocate slightly to handle the race where the xattr grows
        // between the size probe and the read (TOCTOU mitigation).
        let bufSize = size + 16
        let buf     = Array.zeroCreate<byte> bufSize
        let read    = getxattr(filePath, attrName, buf, unativeint bufSize, 0u, 0)
        if read < 0 then
            let errno = Marshal.GetLastWin32Error()
            Error $"getxattr read errno %d{errno}"
        else
            Ok (buf.[0 .. read - 1] |> Some)

// Binary plist spec: https://github.com/opensource-apple/CF/blob/master/CFBinaryPList.cc
/// Minimal bplist00 parser — handles only the subset used for tag arrays:
/// a top-level array of UTF-8/UTF-16BE strings (which is all macOS ever writes here).
let private parseBinaryPlist (data: byte[]) : Result<string list, string> =
    try
        // bplist00 magic header
        if data.Length < 8 || data.[0..5] <> "bplist"B then
            Error "Not a binary plist"
        else

        // --- Trailer (last 32 bytes) ---
        let trailerOffset        = data.Length - 32
        let offsetTableOffsetSize = int data.[trailerOffset + 6]
        let objectRefSize         = int data.[trailerOffset + 7]
        // All multi-byte integers in bplist are big-endian
        let readBigEndianInt64 (slice: byte[]) =
            BitConverter.ToInt64(slice |> Array.rev, 0)

        let topObject         = readBigEndianInt64 data.[trailerOffset + 16 .. trailerOffset + 23]
        let offsetTableStart  = readBigEndianInt64 data.[trailerOffset + 24 .. trailerOffset + 31]

        /// Read a variable-width big-endian integer stored in `offsetTableOffsetSize` bytes.
        let readOffsetTableEntry idx =
            let pos    = int offsetTableStart + idx * offsetTableOffsetSize
            let bytes  = data.[pos .. pos + offsetTableOffsetSize - 1] |> Array.rev
            let padded = Array.zeroCreate<byte> 8
            bytes |> Array.iteri (fun i b -> padded.[i] <- b)
            int (BitConverter.ToInt64(padded, 0))

        /// Read a bplist integer object at `offset`; returns (value, bytesConsumed).
        /// Integer marker: 0x1N where N = power-of-two byte count (0→1B, 1→2B, 2→4B, 3→8B).
        let readIntObject offset =
            let marker  = data.[offset]
            if marker &&& 0xF0uy <> 0x10uy then
                Error $"Expected int object at offset %d{offset}, got 0x%02X{marker}"
            else
                let byteCount = 1 <<< int (marker &&& 0x0Fuy)
                let bytes     = data.[offset + 1 .. offset + byteCount] |> Array.rev
                let padded    = Array.zeroCreate<byte> 8
                bytes |> Array.iteri (fun i b -> padded.[i] <- b)
                Ok (int (BitConverter.ToInt64(padded, 0)), 1 + byteCount)

        /// Decode a count/length field that may use the bplist continuation marker (0x0F nibble).
        /// Returns (count, nextByteOffset) where nextByteOffset is the byte after the count.
        let readCountAt offset nibble =
            if nibble <> 0x0F then
                Ok (nibble, offset)
            else
                readIntObject offset
                |> Result.map (fun (count, consumed) -> count, offset + consumed)

        /// Read a string object at `offset`; returns the tag string (colour suffix stripped).
        /// Returns None for unrecognised object types so callers can skip gracefully.
        let readStringObject offset : Result<string option, string> =
            let marker  = data.[offset]
            let objType = marker &&& 0xF0uy
            let nibble  = int (marker &&& 0x0Fuy)
            match objType with
            | 0x50uy ->  // ASCII string
                readCountAt (offset + 1) nibble
                |> Result.map (fun (charCount, dataStart) ->
                    let s = Text.Encoding.ASCII.GetString(data, dataStart, charCount)
                    // Strip macOS colour suffix e.g. "Work\n6"
                    Some s)

            | 0x60uy ->  // Unicode string (UTF-16BE)
                readCountAt (offset + 1) nibble
                |> Result.map (fun (charCount, dataStart) ->
                    let chars = Array.zeroCreate<char> charCount
                    for i in 0 .. charCount - 1 do
                        chars.[i] <- char ((int data.[dataStart + i * 2] <<< 8)
                                           ||| int data.[dataStart + i * 2 + 1])
                    // Strip colour suffix
                    Some (String(chars)))

            | _ -> // skip
                Ok None

        // --- Locate and decode the top-level array ---
        let topOffset = readOffsetTableEntry (int topObject)
        let topMarker = data.[topOffset]
        if topMarker &&& 0xF0uy <> 0xA0uy then
            Error "Top object is not an array"
        else

        let arrayNibble = int (topMarker &&& 0x0Fuy)
        match readCountAt (topOffset + 1) arrayNibble with
        | Error e -> Error e
        | Ok (arrayCount, refsStart) ->

        // Accumulate tags, collecting any inner errors
        let mutable innerError : string option = None
        let values =
            [ for i in 0 .. arrayCount - 1 do
                if innerError.IsNone then
                    let refPos   = refsStart + i * objectRefSize
                    let refBytes = data.[refPos .. refPos + objectRefSize - 1] |> Array.rev
                    let padded   = Array.zeroCreate<byte> 8
                    refBytes |> Array.iteri (fun j b -> padded.[j] <- b)
                    let objIdx   = int (BitConverter.ToInt64(padded, 0))
                    match readStringObject (readOffsetTableEntry objIdx) with
                    | Error e         -> innerError <- Some e
                    | Ok None         -> ()          // Unknown type — skip silently
                    | Ok (Some s)     -> yield s ]

        match innerError with
        | Some e -> Error e
        | None   -> Ok values

    with ex -> Error ex.Message
    
[<SupportedOSPlatform("macos")>]
let getStringList attrName filePath =
    match filePath |> readBytes attrName  with
    | Ok(Some bytes) ->
        bytes
        |> parseBinaryPlist
        |> Result.defaultValue []
    | _ -> []
