namespace IsomFolio.Core.Metadata

open IsomFolio.Core.Metadata.Mac.Types
open IsomFolio.Core.Metadata.Xmp.Types

type FileMetadata = {
    XmpSidecar: XmpMetadata option
    XmpEmbedded: XmpMetadata option
    AppleMetadata: AppleMetadata option
}
