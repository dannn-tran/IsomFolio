namespace IsomFolio.Core.Metadata.Mac

open System
open System.Runtime.Versioning

type Tag = {
    Text: string
    ColorIdx: int
}

type AppleMetadata = {
    UserTags: Tag list
}

module AppleMetadata =
    let private (|Int|_|) (s: string) = match Int32.TryParse(s) with true, i -> Some i | _ -> None

    let private parseTag(s: string): Tag =
        match s.Split('\n', 2) with
        | [| text; Int colorIdx |] -> { Text = text; ColorIdx = colorIdx }
        | _ -> { Text = s; ColorIdx = 0 }

    [<SupportedOSPlatform("macos")>]
    let fromFilePath (filePath: string): AppleMetadata =
        {
            UserTags = filePath
            |> XAttrReader.getStringList Constants.UserTags
            |> List.map parseTag
        }