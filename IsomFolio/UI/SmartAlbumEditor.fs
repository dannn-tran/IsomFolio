module IsomFolio.UI.SmartAlbumEditor

open System
open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open IsomFolio.Core.Models
open IsomFolio.Core.PathUtils
open IsomFolio.Core.Indexing.FolderTree
open IsomFolio.UI.ContextMenuExt
open IsomFolio.UI.SearchBar

type State = {
    AlbumId      : AlbumId
    AlbumName    : string
    TagInput     : string
    TagFilter    : string list
    DateFrom     : string
    DateTo       : string
    ExtFilter    : string list
    FolderFilter : string option
}

type Msg =
    | TagInputChanged of string
    | TagAdded        of string
    | TagRemoved      of string
    | DateFromChanged of string
    | DateToChanged   of string
    | ExtToggled      of string
    | FolderFilterSet of string option
    | SaveRequested
    | Cancelled

let initFromAlbum (album: Album) =
    match album.Kind with
    | Smart q ->
        {
            AlbumId      = album.Id
            AlbumName    = album.Name
            TagInput     = ""
            TagFilter    = q.Tags
            DateFrom     = q.DateRange |> Option.map (fun (f, _) -> f.ToString("yyyy-MM-dd")) |> Option.defaultValue ""
            DateTo       = q.DateRange |> Option.map (fun (_, t) -> t.ToString("yyyy-MM-dd")) |> Option.defaultValue ""
            ExtFilter    = q.Extensions
            FolderFilter = q.FolderPath
        }
    | Manual -> failwith "Cannot edit criteria for a manual album"

let toSearchQuery (state: State) : SearchQuery =
    {
        Text       = None
        FolderPath = state.FolderFilter
        Tags       = state.TagFilter
        Extensions = state.ExtFilter
        DateRange  =
            match parseDateOpt state.DateFrom, parseDateOpt state.DateTo with
            | None, None -> None
            | df, dt     -> Some (df |> Option.defaultValue DateTime.MinValue, dt |> Option.defaultValue DateTime.MaxValue)
        SortBy     = Date
        SortAsc    = false
    }

let update (msg: Msg) (state: State) =
    match msg with
    | TagInputChanged txt -> { state with TagInput = txt }
    | TagAdded tag ->
        let tag = tag.Trim()
        if tag = "" || state.TagFilter |> List.exists (fun t -> String.Equals(t, tag, StringComparison.OrdinalIgnoreCase))
        then { state with TagInput = "" }
        else { state with TagFilter = state.TagFilter @ [ tag ]; TagInput = "" }
    | TagRemoved tag ->
        { state with TagFilter = state.TagFilter |> List.filter (fun t -> not (String.Equals(t, tag, StringComparison.OrdinalIgnoreCase))) }
    | DateFromChanged s   -> { state with DateFrom = s }
    | DateToChanged s     -> { state with DateTo = s }
    | ExtToggled ext ->
        let newExt =
            if state.ExtFilter |> List.contains ext then state.ExtFilter |> List.filter ((<>) ext)
            else state.ExtFilter @ [ ext ]
        { state with ExtFilter = newExt }
    | FolderFilterSet f   -> { state with FolderFilter = f }
    | SaveRequested | Cancelled -> state

let private knownExts = [ "jpg"; "jpeg"; "png"; "gif"; "webp"; "tiff"; "heic"; "raw" ]
let private labelWidth  = 80.0

let private tagChip (tag: string) (dispatch: Msg -> unit) =
    Border.create [
        Border.background (SolidColorBrush(Theme.accent))
        Border.cornerRadius 3.0
        Border.margin (Avalonia.Thickness(0.0, 0.0, 4.0, 2.0))
        Border.child (
            StackPanel.create [
                StackPanel.orientation Orientation.Horizontal
                StackPanel.margin (Avalonia.Thickness(5.0, 1.0))
                StackPanel.children [
                    TextBlock.create [
                        TextBlock.text tag
                        TextBlock.foreground Brushes.White
                        TextBlock.fontSize Theme.FontSize.xs
                        TextBlock.verticalAlignment VerticalAlignment.Center
                    ] :> Avalonia.FuncUI.Types.IView
                    Button.create [
                        Button.content "×"
                        Button.fontSize 10.0
                        Button.padding (Avalonia.Thickness(2.0, 0.0))
                        Button.background Brushes.Transparent
                        Button.borderThickness (Avalonia.Thickness(0.0))
                        Button.foreground Brushes.White
                        Button.onClick((fun _ -> dispatch (TagRemoved tag)), SubPatchOptions.OnChangeOf tag)
                    ] :> Avalonia.FuncUI.Types.IView
                ]
            ])
    ]

let private extChip (ext: string) (selected: bool) (dispatch: Msg -> unit) =
    Border.create [
        Border.cornerRadius 3.0
        Border.margin (Avalonia.Thickness(0.0, 0.0, 4.0, 2.0))
        Border.background (SolidColorBrush(if selected then Theme.accent else Theme.tagChipBg))
        Border.cursor Avalonia.Input.Cursor.Default
        Border.onPointerPressed((fun _ -> dispatch (ExtToggled ext)), SubPatchOptions.OnChangeOf ext)
        Border.child (
            TextBlock.create [
                TextBlock.text ext
                TextBlock.foreground Brushes.White
                TextBlock.fontSize Theme.FontSize.xs
                TextBlock.margin (Avalonia.Thickness(5.0, 2.0))
            ])
    ]

let view (state: State) (dispatch: Msg -> unit) (availableTags: string list) (rootFolders: string list) =
    Grid.create [
        Grid.background (SolidColorBrush(Color.FromArgb(200uy, 0uy, 0uy, 0uy)))
        Grid.children [
            Border.create [
                Border.maxWidth 540.0
                Border.background (SolidColorBrush(Theme.panelBg))
                Border.cornerRadius 6.0
                Border.padding (Avalonia.Thickness(20.0))
                Border.horizontalAlignment HorizontalAlignment.Center
                Border.verticalAlignment VerticalAlignment.Center
                Border.child (
                    StackPanel.create [
                        StackPanel.spacing 12.0
                        StackPanel.children [
                            // Header
                            TextBlock.create [
                                TextBlock.text $"Edit Smart Album: {state.AlbumName}"
                                TextBlock.fontSize Theme.FontSize.lg
                                TextBlock.foreground Brushes.White
                                TextBlock.fontWeight FontWeight.SemiBold
                            ] :> Avalonia.FuncUI.Types.IView
                            // Tags row
                            DockPanel.create [
                                DockPanel.children [
                                    TextBlock.create [
                                        TextBlock.dock Dock.Left
                                        TextBlock.text "Tags"
                                        TextBlock.width labelWidth
                                        TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                        TextBlock.fontSize Theme.FontSize.sm
                                        TextBlock.verticalAlignment VerticalAlignment.Top
                                        TextBlock.margin (Avalonia.Thickness(0.0, 4.0, 0.0, 0.0))
                                    ]
                                    StackPanel.create [
                                        StackPanel.spacing 4.0
                                        StackPanel.children [
                                            WrapPanel.create [
                                                WrapPanel.children [
                                                    for tag in state.TagFilter do
                                                        yield tagChip tag dispatch :> Avalonia.FuncUI.Types.IView
                                                    yield TextBox.create [
                                                        TextBox.text state.TagInput
                                                        TextBox.watermark "add tag…"
                                                        TextBox.width 110.0
                                                        TextBox.fontSize Theme.FontSize.xs
                                                        TextBox.onTextChanged(TagInputChanged >> dispatch)
                                                        TextBox.onKeyDown (fun e ->
                                                            if e.Key = Avalonia.Input.Key.Enter && state.TagInput.Trim() <> "" then
                                                                e.Handled <- true
                                                                dispatch (TagAdded state.TagInput))
                                                    ] :> Avalonia.FuncUI.Types.IView
                                                ]
                                            ] :> Avalonia.FuncUI.Types.IView
                                            let suggestions =
                                                if state.TagInput.Trim() = "" then []
                                                else
                                                    availableTags
                                                    |> List.filter (fun t ->
                                                        t.Contains(state.TagInput.Trim(), StringComparison.OrdinalIgnoreCase)
                                                        && not (state.TagFilter |> List.exists (fun f -> String.Equals(f, t, StringComparison.OrdinalIgnoreCase))))
                                                    |> List.truncate 6
                                            if not suggestions.IsEmpty then
                                                WrapPanel.create [
                                                    WrapPanel.children [
                                                        for tag in suggestions do
                                                            yield Border.create [
                                                                Border.background (SolidColorBrush(Theme.surfaceBg))
                                                                Border.cornerRadius 3.0
                                                                Border.margin (Avalonia.Thickness(0.0, 0.0, 4.0, 2.0))
                                                                Border.cursor Avalonia.Input.Cursor.Default
                                                                Border.onPointerPressed(
                                                                    (fun _ -> dispatch (TagAdded tag)),
                                                                    SubPatchOptions.OnChangeOf tag)
                                                                Border.child (
                                                                    TextBlock.create [
                                                                        TextBlock.text tag
                                                                        TextBlock.foreground (SolidColorBrush(Theme.textDim))
                                                                        TextBlock.fontSize Theme.FontSize.xs
                                                                        TextBlock.margin (Avalonia.Thickness(5.0, 2.0))
                                                                    ])
                                                            ] :> Avalonia.FuncUI.Types.IView
                                                    ]
                                                ] :> Avalonia.FuncUI.Types.IView
                                        ]
                                    ]
                                ]
                            ] :> Avalonia.FuncUI.Types.IView
                            // Date row
                            DockPanel.create [
                                DockPanel.children [
                                    TextBlock.create [
                                        TextBlock.dock Dock.Left
                                        TextBlock.text "Date"
                                        TextBlock.width labelWidth
                                        TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                        TextBlock.fontSize Theme.FontSize.sm
                                        TextBlock.verticalAlignment VerticalAlignment.Center
                                    ]
                                    StackPanel.create [
                                        StackPanel.orientation Orientation.Horizontal
                                        StackPanel.spacing 6.0
                                        StackPanel.children [
                                            TextBox.create [
                                                TextBox.text state.DateFrom
                                                TextBox.watermark "from yyyy-MM-dd"
                                                TextBox.width 130.0
                                                TextBox.fontSize Theme.FontSize.xs
                                                TextBox.onTextChanged(DateFromChanged >> dispatch)
                                            ] :> Avalonia.FuncUI.Types.IView
                                            TextBlock.create [
                                                TextBlock.text "–"
                                                TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                                TextBlock.verticalAlignment VerticalAlignment.Center
                                            ] :> Avalonia.FuncUI.Types.IView
                                            TextBox.create [
                                                TextBox.text state.DateTo
                                                TextBox.watermark "to yyyy-MM-dd"
                                                TextBox.width 130.0
                                                TextBox.fontSize Theme.FontSize.xs
                                                TextBox.onTextChanged(DateToChanged >> dispatch)
                                            ] :> Avalonia.FuncUI.Types.IView
                                        ]
                                    ]
                                ]
                            ] :> Avalonia.FuncUI.Types.IView
                            // Extension row
                            DockPanel.create [
                                DockPanel.children [
                                    TextBlock.create [
                                        TextBlock.dock Dock.Left
                                        TextBlock.text "Extension"
                                        TextBlock.width labelWidth
                                        TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                        TextBlock.fontSize Theme.FontSize.sm
                                        TextBlock.verticalAlignment VerticalAlignment.Center
                                    ]
                                    WrapPanel.create [
                                        WrapPanel.children [
                                            for ext in knownExts do
                                                yield extChip ext (state.ExtFilter |> List.contains ext) dispatch
                                                      :> Avalonia.FuncUI.Types.IView
                                        ]
                                    ]
                                ]
                            ] :> Avalonia.FuncUI.Types.IView
                            // Folder row
                            if not rootFolders.IsEmpty then
                                let folderLabel = state.FolderFilter |> Option.map displayName |> Option.defaultValue "Any folder"
                                DockPanel.create [
                                    DockPanel.children [
                                        TextBlock.create [
                                            TextBlock.dock Dock.Left
                                            TextBlock.text "Folder"
                                            TextBlock.width labelWidth
                                            TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                            TextBlock.fontSize Theme.FontSize.sm
                                            TextBlock.verticalAlignment VerticalAlignment.Center
                                        ]
                                        StackPanel.create [
                                            StackPanel.orientation Orientation.Horizontal
                                            StackPanel.spacing 4.0
                                            StackPanel.children [
                                                Border.create [
                                                    Border.background (SolidColorBrush(Theme.surfaceBg))
                                                    Border.cornerRadius 3.0
                                                    Border.padding (Avalonia.Thickness(6.0, 2.0))
                                                    XBorder.dropdownMenu (
                                                        XContextMenu.create [
                                                            XContextMenu.viewItems [
                                                                yield XMenuItem.create [
                                                                    XMenuItem.header "Any folder"
                                                                    XMenuItem.onClick (fun _ -> dispatch (FolderFilterSet None))
                                                                ]
                                                                for folder in rootFolders do
                                                                    yield XMenuItem.create [
                                                                        XMenuItem.header (displayName folder)
                                                                        XMenuItem.onClick (fun _ -> dispatch (FolderFilterSet (Some folder)))
                                                                    ]
                                                            ]
                                                        ])
                                                    Border.child (
                                                        TextBlock.create [
                                                            TextBlock.text $"{folderLabel} ▾"
                                                            TextBlock.foreground Brushes.White
                                                            TextBlock.fontSize Theme.FontSize.xs
                                                        ])
                                                ] :> Avalonia.FuncUI.Types.IView
                                                if state.FolderFilter.IsSome then
                                                    Button.create [
                                                        Button.content "×"
                                                        Button.fontSize 10.0
                                                        Button.padding (Avalonia.Thickness(4.0, 0.0))
                                                        Button.background Brushes.Transparent
                                                        Button.borderThickness (Avalonia.Thickness(0.0))
                                                        Button.foreground (SolidColorBrush(Theme.textMuted))
                                                        Button.onClick (fun _ -> dispatch (FolderFilterSet None))
                                                    ] :> Avalonia.FuncUI.Types.IView
                                            ]
                                        ]
                                    ]
                                ] :> Avalonia.FuncUI.Types.IView
                            // Buttons
                            DockPanel.create [
                                DockPanel.children [
                                    Button.create [
                                        Button.dock Dock.Right
                                        Button.content "Save"
                                        Button.padding (Avalonia.Thickness(16.0, 6.0))
                                        Button.onClick (fun _ -> dispatch SaveRequested)
                                    ] :> Avalonia.FuncUI.Types.IView
                                    Button.create [
                                        Button.dock Dock.Right
                                        Button.content "Cancel"
                                        Button.padding (Avalonia.Thickness(16.0, 6.0))
                                        Button.margin (Avalonia.Thickness(0.0, 0.0, 8.0, 0.0))
                                        Button.background Brushes.Transparent
                                        Button.onClick (fun _ -> dispatch Cancelled)
                                    ] :> Avalonia.FuncUI.Types.IView
                                ]
                            ] :> Avalonia.FuncUI.Types.IView
                        ]
                    ])
            ]
        ]
    ]
