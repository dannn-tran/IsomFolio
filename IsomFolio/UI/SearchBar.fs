module IsomFolio.UI.SearchBar

open System
open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open IsomFolio.Core.PathUtils
open IsomFolio.Core.Indexing.FolderTree
open IsomFolio.UI.ContextMenuExt

type State = {
    InputText    : string
    CriteriaOpen : bool
    TagInput     : string
    TagFilter    : string list
    DateFrom     : string        // "yyyy-MM-dd" or ""
    DateTo       : string
    ExtFilter    : string list
    FolderFilter : string option // root folder path
}

type Msg =
    | TextChanged             of string
    | QuerySubmitted          of string
    | CriteriaToggled
    | TagInputChanged         of string
    | TagAdded                of string
    | TagRemoved              of string
    | DateFromChanged         of string
    | DateToChanged           of string
    | ExtToggled              of string
    | FolderFilterSet         of string option
    | SaveAsSmartAlbumRequested

let init () = {
    InputText = ""; CriteriaOpen = false; TagInput = ""
    TagFilter = []; DateFrom = ""; DateTo = ""
    ExtFilter = []; FolderFilter = None
}

let hasCriteria (state: State) =
    not state.TagFilter.IsEmpty || state.DateFrom <> "" || state.DateTo <> ""
    || not state.ExtFilter.IsEmpty || state.FolderFilter.IsSome

let parseDateOpt (s: string) : DateTime option =
    let s = s.Trim()
    if s = "" then None
    else
        match DateTime.TryParseExact(s, "yyyy-MM-dd", Globalization.CultureInfo.InvariantCulture, Globalization.DateTimeStyles.None) with
        | true, d -> Some d
        | _ -> None

let isCriteriaMsg = function
    | TagAdded _ | TagRemoved _ | DateFromChanged _ | DateToChanged _ | ExtToggled _ | FolderFilterSet _ -> true
    | _ -> false

let update (msg: Msg) (state: State) =
    match msg with
    | TextChanged txt     -> { state with InputText = txt }
    | QuerySubmitted _    -> state
    | CriteriaToggled     -> { state with CriteriaOpen = not state.CriteriaOpen }
    | TagInputChanged txt -> { state with TagInput = txt }
    | TagAdded tag ->
        let tag = tag.Trim()
        if tag = "" || state.TagFilter |> List.exists (fun t -> String.Equals(t, tag, StringComparison.OrdinalIgnoreCase))
        then { state with TagInput = "" }
        else { state with TagFilter = state.TagFilter @ [ tag ]; TagInput = ""; CriteriaOpen = true }
    | TagRemoved tag ->
        { state with TagFilter = state.TagFilter |> List.filter (fun t -> not (String.Equals(t, tag, StringComparison.OrdinalIgnoreCase))) }
    | DateFromChanged s   -> { state with DateFrom = s; CriteriaOpen = true }
    | DateToChanged s     -> { state with DateTo = s; CriteriaOpen = true }
    | ExtToggled ext ->
        let newExt =
            if state.ExtFilter |> List.contains ext then state.ExtFilter |> List.filter ((<>) ext)
            else state.ExtFilter @ [ ext ]
        { state with ExtFilter = newExt; CriteriaOpen = true }
    | FolderFilterSet f   -> { state with FolderFilter = f; CriteriaOpen = true }
    | SaveAsSmartAlbumRequested -> state

let private knownExts = [ "jpg"; "jpeg"; "png"; "gif"; "webp"; "tiff"; "heic"; "raw" ]
let private labelWidth  = 70.0
let private rowLabelFg  () = SolidColorBrush(Theme.textMuted)

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

let private criteriaPanel (state: State) (availableTags: string list) (rootFolders: string list) (showSave: bool) (dispatch: Msg -> unit) =
    Border.create [
        Border.background (SolidColorBrush(Theme.panelBg))
        Border.borderThickness (Avalonia.Thickness(0.0, 0.0, 0.0, 1.0))
        Border.borderBrush (SolidColorBrush(Theme.tagChipBg))
        Border.padding (Avalonia.Thickness(8.0, 6.0))
        Border.child (
            StackPanel.create [
                StackPanel.spacing 5.0
                StackPanel.children [
                    // Tags row
                    DockPanel.create [
                        DockPanel.children [
                            TextBlock.create [
                                TextBlock.dock Dock.Left
                                TextBlock.text "Tags"
                                TextBlock.width labelWidth
                                TextBlock.foreground (rowLabelFg ())
                                TextBlock.fontSize Theme.FontSize.sm
                                TextBlock.verticalAlignment VerticalAlignment.Top
                                TextBlock.margin (Avalonia.Thickness(0.0, 4.0, 0.0, 0.0))
                            ]
                            StackPanel.create [
                                StackPanel.spacing 4.0
                                StackPanel.children [
                                    // Selected tag chips + input
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
                                    // Tag suggestions (when typing)
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
                                            WrapPanel.margin (Avalonia.Thickness(0.0, 2.0, 0.0, 0.0))
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
                                TextBlock.foreground (rowLabelFg ())
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
                                        TextBlock.foreground (rowLabelFg ())
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
                                TextBlock.foreground (rowLabelFg ())
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
                                    TextBlock.foreground (rowLabelFg ())
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
                    // Footer: Save as Smart Album
                    if showSave then
                        DockPanel.create [
                            DockPanel.margin (Avalonia.Thickness(0.0, 2.0, 0.0, 0.0))
                            DockPanel.children [
                                Button.create [
                                    Button.dock Dock.Right
                                    Button.content "Save as Smart Album…"
                                    Button.fontSize Theme.FontSize.xs
                                    Button.padding (Avalonia.Thickness(8.0, 4.0))
                                    Button.onClick (fun _ -> dispatch SaveAsSmartAlbumRequested)
                                ]
                            ]
                        ] :> Avalonia.FuncUI.Types.IView
                ]
            ])
    ]

let view (state: State) (dispatch: Msg -> unit) (availableTags: string list) (rootFolders: string list) (showSaveAsSmartAlbum: bool) =
    let isOpen = state.CriteriaOpen || hasCriteria state
    let criteriaLabel =
        if hasCriteria state then "Criteria •"
        elif isOpen         then "Criteria ▴"
        else                     "Criteria ▾"
    StackPanel.create [
        StackPanel.children [
            DockPanel.create [
                DockPanel.height 40.0
                DockPanel.background (SolidColorBrush(Theme.panelBg))
                DockPanel.children [
                    Button.create [
                        Button.dock Dock.Right
                        Button.content criteriaLabel
                        Button.fontSize Theme.FontSize.sm
                        Button.foreground (SolidColorBrush(if hasCriteria state then Theme.accent else Theme.textMuted))
                        Button.background Brushes.Transparent
                        Button.borderThickness (Avalonia.Thickness(0.0))
                        Button.padding (Avalonia.Thickness(8.0, 0.0))
                        Button.onClick (fun _ -> dispatch CriteriaToggled)
                    ]
                    TextBox.create [
                        TextBox.watermark "Search files and tags…"
                        TextBox.text state.InputText
                        TextBox.horizontalAlignment HorizontalAlignment.Stretch
                        TextBox.onTextChanged (fun t ->
                            dispatch (TextChanged t))
                        TextBox.onKeyDown (fun e ->
                            if e.Key = Avalonia.Input.Key.Enter then
                                e.Handled <- true
                                dispatch (QuerySubmitted state.InputText))
                    ]
                ]
            ] :> Avalonia.FuncUI.Types.IView
            if isOpen then
                criteriaPanel state availableTags rootFolders showSaveAsSmartAlbum dispatch
                :> Avalonia.FuncUI.Types.IView
        ]
    ]
