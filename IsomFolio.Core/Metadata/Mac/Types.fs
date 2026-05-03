module IsomFolio.Core.Metadata.Mac.Types

type Tag = {
    Text: string
    ColorIdx: int
}

type AppleMetadata = {
    UserTags: Tag list
}
