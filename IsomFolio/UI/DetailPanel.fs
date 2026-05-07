module IsomFolio.UI.DetailPanel

open System
open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open IsomFolio.Core.Models
open IsomFolio.Core.Metadata
open IsomFolio.UI.ContextMenuExt

type State = {
    File            : AssetFile option
    TagTree         : TagTree.State
    IsVisible       : bool
    EmbeddedMeta    : EmbeddedMetadata option
    SourceView      : MetadataSources option
    SourceViewStale : bool
    ShowSourceView  : bool
}

type Msg =
    | FileSelected       of AssetFile
    | TagsLoaded         of string list
    | TagTreeMsg         of TagTree.Msg
    | Closed
    | MetadataLoaded     of EmbeddedMetadata option
    | MetadataViewToggled
    | SourceViewRequested
    | SourceViewLoaded   of MetadataSources
    | SourceViewFailed   of exn
    | TagBrowserRequested

let init () = {
    File            = None
    TagTree         = TagTree.init ()
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
                TagTree         = TagTree.init ()
                IsVisible       = true
                EmbeddedMeta    = None
                SourceView      = None
                SourceViewStale = false
                ShowSourceView  = false }
    | TagsLoaded ts        -> { state with TagTree = TagTree.fromTagList ts }
    | TagTreeMsg tMsg      -> { state with TagTree = TagTree.update tMsg state.TagTree }
    | Closed               -> { state with IsVisible = false }
    | MetadataLoaded meta  -> { state with EmbeddedMeta = meta }
    | MetadataViewToggled  ->
        { state with
            ShowSourceView  = not state.ShowSourceView
            SourceView      = None
            SourceViewStale = false }
    | SourceViewRequested  -> state
    | SourceViewLoaded sources ->
        let stale =
            match state.EmbeddedMeta with
            | None -> false
            | Some cached -> EmbeddedMetadata.ofSources sources <> cached
        { state with SourceView = Some sources; SourceViewStale = stale }
    | SourceViewFailed _   -> state
    | TagBrowserRequested  -> state  // handled in MainView

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

let private aggregatedSection (meta: EmbeddedMetadata option) (tagTree: TagTree.State) (dispatch: Msg -> unit) =
    let xmpCore = meta |> Option.bind (fun m -> m.Xmp) |> Option.map (fun x -> x.Core)
    let xmpDc   = meta |> Option.bind (fun m -> m.Xmp) |> Option.map (fun x -> x.DublinCore)
    let apple   = meta |> Option.bind (fun m -> m.AppleMetadata)

    let optRow label value =
        value |> Option.map (fun v -> metaRow label v) |> Option.toList

    StackPanel.create [
        StackPanel.children [
            yield! (xmpCore |> Option.bind (fun x -> x.Rating) |> Option.map (fun r -> metaRow "Rating" (String.replicate r "★" + String.replicate (5 - r) "☆")) |> Option.toList)
            yield! (optRow "Label"  (xmpCore |> Option.bind (fun x -> x.Label)))
            yield! (optRow "Title"  (xmpDc   |> Option.bind (fun x -> x.Title)))
            yield! (optRow "Notes"  (xmpDc   |> Option.bind (fun x -> x.Description)))
            yield! (match xmpDc |> Option.map (fun x -> x.Creator) |> Option.defaultValue [] with
                    | [] -> []
                    | cs -> [ metaRow "Creator" (String.concat ", " cs) ])
            match apple with
            | Some a when not a.UserTags.IsEmpty ->
                yield metaRow "OS Tags" (a.UserTags |> List.map (fun t -> t.Text) |> String.concat ", ")
            | _ -> ()
            yield DockPanel.create [
                DockPanel.margin (Avalonia.Thickness(0.0, 12.0, 0.0, 4.0))
                DockPanel.children [
                    Button.create [
                        Button.dock Dock.Right
                        Button.content "Browse all"
                        Button.fontSize Theme.FontSize.xs
                        Button.background Brushes.Transparent
                        Button.foreground (SolidColorBrush(Theme.textMuted))
                        Button.padding (Avalonia.Thickness(4.0, 0.0, 0.0, 0.0))
                        Button.onClick (fun _ -> dispatch TagBrowserRequested)
                    ]
                    TextBlock.create [
                        TextBlock.text "TAGS"
                        TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                        TextBlock.fontSize Theme.FontSize.xs
                        TextBlock.verticalAlignment VerticalAlignment.Center
                    ]
                ]
            ] :> Avalonia.FuncUI.Types.IView
            yield TagTree.view tagTree (TagTreeMsg >> dispatch)
        ]
    ] :> Avalonia.FuncUI.Types.IView

let private detailsSection (sources: MetadataSources) (stale: bool) =
    let xmpRows (xmp: Xmp.XmpMetadata) : Avalonia.FuncUI.Types.IView list =
        let core = xmp.Core
        let dc   = xmp.DublinCore
        [
            yield! (core.Rating |> Option.map (fun r -> metaRow "Rating" (String.replicate r "★" + String.replicate (5 - r) "☆")) |> Option.toList)
            yield! (core.Label |> Option.map (fun l -> metaRow "Label" l) |> Option.toList)
            yield! (dc.Title |> Option.map (fun t -> metaRow "Title" t) |> Option.toList)
            yield! (dc.Description |> Option.map (fun d -> metaRow "Notes" d) |> Option.toList)
            yield! (match dc.Creator with [] -> [] | cs -> [ metaRow "Creator" (String.concat ", " cs) ])
            yield! (match dc.Subject  with [] -> [] | ss -> [ metaRow "Subjects" (String.concat ", " ss) ])
        ]

    StackPanel.create [
        StackPanel.children [
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
            match sources.Sidecar with
            | Some xmp ->
                yield sectionHeader "SIDECAR XMP"
                yield! xmpRows xmp
            | None -> ()
            match sources.Embedded with
            | Some xmp ->
                yield sectionHeader "EMBEDDED XMP"
                yield! xmpRows xmp
            | None -> ()
            match sources.Apple with
            | Some a when not a.UserTags.IsEmpty ->
                yield sectionHeader "APPLE METADATA"
                yield metaRow "Tags" (a.UserTags |> List.map (fun t -> t.Text) |> String.concat ", ")
            | _ -> ()
            yield sectionHeader "FILESYSTEM"
            yield metaRow "Created"  (sources.FileSystem.CreatedAt.LocalDateTime.ToString("yyyy-MM-dd HH:mm"))
            yield metaRow "Modified" (sources.FileSystem.ModifiedAt.LocalDateTime.ToString("yyyy-MM-dd HH:mm"))
            yield metaRow "Size"     (formatBytes sources.FileSystem.SizeBytes)
        ]
    ] :> Avalonia.FuncUI.Types.IView

let private metadataHeader (showDetails: bool) (dispatch: Msg -> unit) =
    DockPanel.create [
        DockPanel.margin (Avalonia.Thickness(0.0, 12.0, 0.0, 4.0))
        DockPanel.children [
            Border.create [
                Border.dock Dock.Right
                Border.padding (Avalonia.Thickness(4.0, 0.0, 0.0, 0.0))
                XBorder.dropdownMenu (
                    XContextMenu.create [
                        XContextMenu.viewItems [
                            XMenuItem.create [
                                XMenuItem.header (if showDetails then "View summary" else "View details")
                                XMenuItem.onClick (fun _ ->
                                    dispatch MetadataViewToggled
                                    if not showDetails then dispatch SourceViewRequested)
                            ]
                        ]
                    ])
                Border.child (TextBlock.create [
                    TextBlock.text "⋮"
                    TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                    TextBlock.fontSize Theme.FontSize.sm
                    TextBlock.verticalAlignment VerticalAlignment.Center
                ])
            ]
            TextBlock.create [
                TextBlock.text "FILE METADATA"
                TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                TextBlock.fontSize Theme.FontSize.xs
                TextBlock.verticalAlignment VerticalAlignment.Center
            ]
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

                                yield metadataHeader state.ShowSourceView dispatch

                                if state.ShowSourceView then
                                    match state.SourceView with
                                    | None ->
                                        yield TextBlock.create [
                                            TextBlock.text "Loading details…"
                                            TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                            TextBlock.fontSize Theme.FontSize.sm
                                        ] :> Avalonia.FuncUI.Types.IView
                                    | Some sources ->
                                        yield detailsSection sources state.SourceViewStale
                                else
                                    yield aggregatedSection state.EmbeddedMeta state.TagTree dispatch
                            ]
                        ])
                ]
        ]
    ] :> Avalonia.FuncUI.Types.IView
