namespace IsomFolio.Core.Tagging

type TagSource =
    | MacOSNative           // com.apple.metadata:kMDItemUserTags (extended attrs)
    | XmpSidecar of path: string   // .xmp sidecar file
    | EmbeddedXmp           // XMP embedded in JPEG/PDF/etc
    | ExifMetadata          // EXIF subject/keywords

type Tag = {
    Value: string
    Source: TagSource
}

type TagCollection = Tag list