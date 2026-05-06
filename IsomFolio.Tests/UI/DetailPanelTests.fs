module IsomFolio.App.Tests.UI.DetailPanelTests

open System
open Xunit
open IsomFolio.Core.Models
open IsomFolio.Core.Metadata
open IsomFolio.Core.Metadata.Xmp
open IsomFolio.UI

let private makeFile (name: string) : AssetFile =
    let path = $"/photos/{name}.jpg"
    {
        Id            = IsomFolio.Core.FileIndex.computeFileId path
        Path          = path
        Name          = $"{name}.jpg"
        Folder        = "/photos"
        Ext           = "jpg"
        SizeBytes     = 1024L
        MTimeUnix     = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        CreatedAtUnix = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        IsOrphaned    = false
        OrphanedAt    = None
    }

let private makeXmp rating =
    Some {
        Core       = { XmpCore.empty with Rating = Some rating }
        DublinCore = DublinCore.empty
    }

let private makeMeta rating = { Xmp = makeXmp rating; AppleMetadata = None }

let private makeSources sidecar embedded =
    {
        Sidecar    = sidecar
        Embedded   = embedded
        Apple      = None
        FileSystem = { CreatedAt = DateTimeOffset.UtcNow; ModifiedAt = DateTimeOffset.UtcNow; SizeBytes = 4L }
    }

module FileSelected =

    [<Fact>]
    let ``selecting a new file clears metadata and shows panel`` () =
        let state = { DetailPanel.init () with EmbeddedMeta = Some (makeMeta 3) }
        let nextState = DetailPanel.update (DetailPanel.FileSelected (makeFile "beach")) state
        Assert.True(nextState.IsVisible)
        Assert.True(nextState.EmbeddedMeta.IsNone)

    [<Fact>]
    let ``re-selecting the same file preserves cached metadata`` () =
        let f = makeFile "beach"
        let state =
            { DetailPanel.init () with
                File        = Some f
                EmbeddedMeta = Some (makeMeta 3) }
        let nextState = DetailPanel.update (DetailPanel.FileSelected f) state
        Assert.Equal(Some (makeMeta 3), nextState.EmbeddedMeta)


module MetadataLoaded =

    [<Fact>]
    let ``sets EmbeddedMeta on the state`` () =
        let state = DetailPanel.init ()
        let meta = makeMeta 4
        let nextState = DetailPanel.update (DetailPanel.MetadataLoaded (Some meta)) state
        Assert.Equal(Some meta, nextState.EmbeddedMeta)

    [<Fact>]
    let ``None clears EmbeddedMeta`` () =
        let state = { DetailPanel.init () with EmbeddedMeta = Some (makeMeta 2) }
        let nextState = DetailPanel.update (DetailPanel.MetadataLoaded None) state
        Assert.True(nextState.EmbeddedMeta.IsNone)


module MetadataViewToggled =

    [<Fact>]
    let ``flips ShowSourceView`` () =
        let state = DetailPanel.init ()
        Assert.False(state.ShowSourceView)
        let next1 = DetailPanel.update DetailPanel.MetadataViewToggled state
        Assert.True(next1.ShowSourceView)
        let next2 = DetailPanel.update DetailPanel.MetadataViewToggled next1
        Assert.False(next2.ShowSourceView)

    [<Fact>]
    let ``clears SourceView and stale flag on toggle`` () =
        let state =
            { DetailPanel.init () with
                ShowSourceView  = true
                SourceView      = Some (makeSources (makeXmp 5) None)
                SourceViewStale = true }
        let nextState = DetailPanel.update DetailPanel.MetadataViewToggled state
        Assert.True(nextState.SourceView.IsNone)
        Assert.False(nextState.SourceViewStale)


module SourceViewLoaded =

    [<Fact>]
    let ``marks stale when live sources differ from cached metadata`` () =
        let cached = makeMeta 3
        let state = { DetailPanel.init () with EmbeddedMeta = Some cached }
        // Sources resolve (sidecar-wins) to rating 5 — different from cached rating 3
        let sources = makeSources (makeXmp 5) None
        let nextState = DetailPanel.update (DetailPanel.SourceViewLoaded sources) state
        Assert.True(nextState.SourceViewStale)
        Assert.Equal(Some sources, nextState.SourceView)

    [<Fact>]
    let ``not stale when live sources match cached metadata`` () =
        let xmp = makeXmp 3
        let cached = { Xmp = xmp; AppleMetadata = None }
        let state = { DetailPanel.init () with EmbeddedMeta = Some cached }
        // Sources resolve to same rating 3
        let sources = makeSources xmp None
        let nextState = DetailPanel.update (DetailPanel.SourceViewLoaded sources) state
        Assert.False(nextState.SourceViewStale)

    [<Fact>]
    let ``not stale when no cached metadata`` () =
        let state = DetailPanel.init ()
        let sources = makeSources (makeXmp 5) None
        let nextState = DetailPanel.update (DetailPanel.SourceViewLoaded sources) state
        Assert.False(nextState.SourceViewStale)
