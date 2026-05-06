module IsomFolio.UI.DetailPanel

open System
open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open IsomFolio.Core.Models
open IsomFolio.Core.Metadata

type State = {
    File           : AssetFile option
    Tags           : string list
    TagInput       : string
    IsVisible      : bool
    EmbeddedMeta   : EmbeddedMetadata option
    SourceView     : MetadataSources option
    SourceViewStale: bool
    ShowSourceView : bool
}

type Msg =
    | FileSelected       of AssetFile
    | TagsLoaded         of string list
    | TagInputChanged    of string
    | OpenExternally
    | RevealInExplorer
    | Closed
    | MetadataLoaded     of EmbeddedMetadata option
    | MetadataViewToggled
    | SourceViewRequested
    | SourceViewLoaded   of MetadataSources
    | SourceViewFailed   of exn

let init () = {
    File            = None
    Tags            = []
    TagInput        = ""
    IsVisible       = false
    EmbeddedMeta    = None
    SourceView      = None
    SourceViewStale = false
    ShowSourceView  = false
}

let update (msg: Msg) (state: State) =
    match msg with
    | FileSelected f ->
        let sameFile = state.File |> Option.map (fun x -> x.Id = f.Id) = Some true
        if sameFile then
            { state with File = Some f; IsVisible = true }
        else
            { state with
                File            = Some f
                Tags            = []
                TagInput        = ""
                IsVisible       = true
                EmbeddedMeta    = None
                SourceView      = None
                SourceViewStale = false
                ShowSourceView  = false }
    | TagsLoaded ts        -> { state with Tags = ts }
    | TagInputChanged t    -> { state with TagInput = t }
    | Closed               -> { state with IsVisible = false }
    | MetadataLoaded meta  -> { state with EmbeddedMeta = meta }
    | MetadataViewToggled  -> { state with ShowSourceView = not state.ShowSourceView; SourceView = None; SourceViewStale = false }
    | SourceViewRequested  -> state
    | SourceViewLoaded sources ->
        let stale =
            match state.EmbeddedMeta with
            | None -> false
            | Some cached -> EmbeddedMetadata.ofSources sources <> cached
        { state with SourceView = Some sources; SourceViewStale = stale }
    | SourceViewFailed _   -> state
    | OpenExternally
    | RevealInExplorer     -> state   // handled by MainView

let private formatBytes (bytes: int64) =
    if bytes < 1024L then $"{bytes} B"
    elif bytes < 1024L * 1024L then $"{bytes / 1024L} KB"
    else $"{bytes / 1024L / 1024L} MB"

let private formatUnix (unix: int64) =
    if unix = 0L then "—"
    else DateTimeOffset.FromUnixTimeSeconds(unix).LocalDateTime.ToString("yyyy-MM-dd HH:mm")

let private metaRow (label: string) (value: string) =
    DockPanel.create [
        DockPanel.margin (Avalonia.Thickness(0.0, 3.0))
        DockPanel.children [
            TextBlock.create [
                TextBlock.dock Dock.Left
                TextBlock.text label
                TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                TextBlock.fontSize Theme.FontSize.sm
                TextBlock.width 80.0
            ]
            TextBlock.create [
                TextBlock.text value
                TextBlock.foreground Brushes.White
                TextBlock.fontSize Theme.FontSize.sm
                TextBlock.textWrapping TextWrapping.Wrap
            ]
        ]
    ] :> Avalonia.FuncUI.Types.IView

let private sectionHeader (text: string) =
    TextBlock.create [
        TextBlock.text text
        TextBlock.foreground (SolidColorBrush(Theme.textMuted))
        TextBlock.fontSize Theme.FontSize.xs
        TextBlock.margin (Avalonia.Thickness(0.0, 12.0, 0.0, 4.0))
    ] :> Avalonia.FuncUI.Types.IView

let private tagChip (tag: string) (dispatch: Msg -> unit) =
    Border.create [
        Border.cornerRadius 4.0
        Border.background (SolidColorBrush(Theme.tagChipBg))
        Border.margin (Avalonia.Thickness(0.0, 2.0, 4.0, 2.0))
        Border.child (
            StackPanel.create [
                StackPanel.orientation Orientation.Horizontal
                StackPanel.margin (Avalonia.Thickness(6.0, 3.0))
                StackPanel.children [
                    TextBlock.create [
                        TextBlock.text tag
                        TextBlock.foreground Brushes.White
                        TextBlock.fontSize Theme.FontSize.sm
                        TextBlock.verticalAlignment VerticalAlignment.Center
                    ]
                ]
            ])
    ] :> Avalonia.FuncUI.Types.IView

let private metadataSection (meta: EmbeddedMetadata option) (tags: string list) (dispatch: Msg -> unit) =
    let xmpCore = meta |> Option.bind (fun m -> m.Xmp) |> Option.map (fun x -> x.Core)
    let xmpDc   = meta |> Option.bind (fun m -> m.Xmp) |> Option.map (fun x -> x.DublinCore)
    let apple   = meta |> Option.bind (fun m -> m.AppleMetadata)

    let optRow label value =
        value |> Option.map (fun v -> metaRow label v) |> Option.toList

    let allTags =
        let subjects  = xmpDc |> Option.map (fun x -> x.Subject) |> Option.defaultValue []
        let appleTags = apple |> Option.map (fun a -> a.UserTags |> List.map (fun t -> t.Text)) |> Option.defaultValue []
        (tags @ subjects @ appleTags) |> List.distinct

    StackPanel.create [
        StackPanel.children [
            yield sectionHeader "METADATA"
            yield! (xmpCore |> Option.bind (fun x -> x.Rating) |> Option.map (fun r -> metaRow "Rating" (String.replicate r "★" + String.replicate (5 - r) "☆")) |> Option.toList)
            yield! (optRow "Label"  (xmpCore |> Option.bind (fun x -> x.Label)))
            yield! (optRow "Title"  (xmpDc   |> Option.bind (fun x -> x.Title)))
            yield! (optRow "Notes"  (xmpDc   |> Option.bind (fun x -> x.Description)))
            yield! (match xmpDc |> Option.map (fun x -> x.Creator) |> Option.defaultValue [] with
                    | [] -> []
                    | cs -> [ metaRow "Creator" (String.concat ", " cs) ])
            if not allTags.IsEmpty then
                yield sectionHeader "TAGS"
                yield WrapPanel.create [
                    WrapPanel.children [
                        for tag in allTags do
                            yield tagChip tag dispatch
                    ]
                ] :> Avalonia.FuncUI.Types.IView
        ]
    ] :> Avalonia.FuncUI.Types.IView

let private sourceViewSection (sources: MetadataSources) (stale: bool) (dispatch: Msg -> unit) =
    let xmpOpt label (xmp: Xmp.XmpMetadata option) =
        match xmp with
        | None -> metaRow label "—"
        | Some x ->
            let rating = x.Core.Rating |> Option.map string |> Option.defaultValue "—"
            let title  = x.DublinCore.Title |> Option.defaultValue "—"
            metaRow label $"Rating {rating}, Title: {title}"

    StackPanel.create [
        StackPanel.children [
            yield sectionHeader "METADATA SOURCES"
            if stale then
                yield Border.create [
                    Border.background (SolidColorBrush(Colors.DarkOrange))
                    Border.cornerRadius 4.0
                    Border.margin (Avalonia.Thickness(0.0, 4.0))
                    Border.padding (Avalonia.Thickness(8.0, 4.0))
                    Border.child (TextBlock.create [
                        TextBlock.text "Metadata may be outdated"
                        TextBlock.foreground Brushes.White
                        TextBlock.fontSize Theme.FontSize.xs
                    ])
                ] :> Avalonia.FuncUI.Types.IView
            yield xmpOpt "Sidecar" sources.Sidecar
            yield xmpOpt "Embedded" sources.Embedded
            yield metaRow "Created"  (sources.FileSystem.CreatedAt.LocalDateTime.ToString("yyyy-MM-dd HH:mm"))
            yield metaRow "Modified" (sources.FileSystem.ModifiedAt.LocalDateTime.ToString("yyyy-MM-dd HH:mm"))
            yield metaRow "Size"     (formatBytes sources.FileSystem.SizeBytes)
            match sources.Apple with
            | None -> ()
            | Some a ->
                yield metaRow "Tags (OS)" (a.UserTags |> List.map (fun t -> t.Text) |> String.concat ", ")
        ]
    ] :> Avalonia.FuncUI.Types.IView

let view (state: State) (dispatch: Msg -> unit) =
    if not state.IsVisible then Border.create [] :> Avalonia.FuncUI.Types.IView else
    DockPanel.create [
        DockPanel.width 280.0
        DockPanel.background (SolidColorBrush(Theme.panelBg))
        DockPanel.children [
            DockPanel.create [
                DockPanel.dock Dock.Top
                DockPanel.margin (Avalonia.Thickness(8.0, 8.0, 8.0, 0.0))
                DockPanel.children [
                    Button.create [
                        Button.dock Dock.Right
                        Button.content "×"
                        Button.background Brushes.Transparent
                        Button.foreground (SolidColorBrush(Theme.textDim))
                        Button.onClick (fun _ -> dispatch Closed)
                    ]
                    TextBlock.create [
                        TextBlock.text "Details"
                        TextBlock.foreground Brushes.White
                        TextBlock.fontSize Theme.FontSize.lg
                        TextBlock.fontWeight FontWeight.SemiBold
                        TextBlock.verticalAlignment VerticalAlignment.Center
                    ]
                ]
            ]
            match state.File with
            | None ->
                TextBlock.create [
                    TextBlock.text "No file selected"
                    TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                    TextBlock.horizontalAlignment HorizontalAlignment.Center
                    TextBlock.verticalAlignment VerticalAlignment.Center
                ]
            | Some f ->
                ScrollViewer.create [
                    ScrollViewer.content (
                        StackPanel.create [
                            StackPanel.margin (Avalonia.Thickness(12.0, 8.0))
                            StackPanel.children [
                                yield sectionHeader "FILE"
                                yield metaRow "Name"     f.Name
                                yield metaRow "Size"     (formatBytes f.SizeBytes)
                                yield metaRow "Created"  (formatUnix f.CreatedAtUnix)
                                yield metaRow "Modified" (formatUnix f.MTimeUnix)
                                yield metaRow "Path"     f.Folder

                                if state.ShowSourceView then
                                    match state.SourceView with
                                    | None ->
                                        yield TextBlock.create [
                                            TextBlock.text "Loading sources…"
                                            TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                            TextBlock.fontSize Theme.FontSize.sm
                                        ] :> Avalonia.FuncUI.Types.IView
                                    | Some sources ->
                                        yield sourceViewSection sources state.SourceViewStale dispatch
                                else
                                    yield metadataSection state.EmbeddedMeta state.Tags dispatch

                                yield StackPanel.create [
                                    StackPanel.orientation Orientation.Horizontal
                                    StackPanel.margin (Avalonia.Thickness(0.0, 12.0))
                                    StackPanel.children [
                                        Button.create [
                                            Button.content "Open"
                                            Button.margin (Avalonia.Thickness(0.0, 0.0, 4.0, 0.0))
                                            Button.onClick (fun _ -> dispatch OpenExternally)
                                        ]
                                        Button.create [
                                            Button.content "Reveal"
                                            Button.margin (Avalonia.Thickness(0.0, 0.0, 4.0, 0.0))
                                            Button.onClick (fun _ -> dispatch RevealInExplorer)
                                        ]
                                        Button.create [
                                            Button.content (if state.ShowSourceView then "Basic" else "Sources")
                                            Button.onClick (fun _ ->
                                                dispatch MetadataViewToggled
                                                if not state.ShowSourceView then
                                                    dispatch SourceViewRequested)
                                        ]
                                    ]
                                ] :> Avalonia.FuncUI.Types.IView
                            ]
                        ])
                ]
        ]
    ] :> Avalonia.FuncUI.Types.IView
