module IsomFolio.UI.Sidebar

open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open IsomFolio.PathUtils
open IsomFolio.Indexing.FolderTree

type State = {
    Folders      : string list
    FolderTree   : FolderNode list
    Tags         : (string * int) list   // (name, count)
    SelectedTags : string list
    SelectedFolder : string option
}

type Msg =
    | FolderRemoved of string
    | FolderSelected of string
    | TagToggled    of string
    | FoldersLoaded of string list
    | FolderTreeLoaded of FolderNode list
    | TagsLoaded    of (string * int) list

let init () = { Folders = []; FolderTree = []; Tags = []; SelectedTags = []; SelectedFolder = None }

let private isPathWithinRoot (root: string) (path: string) =
    samePath root path
    || path.StartsWith(root + string System.IO.Path.DirectorySeparatorChar, if System.OperatingSystem.IsWindows() then System.StringComparison.OrdinalIgnoreCase else System.StringComparison.Ordinal)

let rec private treeContainsPath (path: string) (nodes: FolderNode list) =
    nodes |> List.exists (fun node ->
        samePath node.Path path || treeContainsPath path node.Children)

let update (msg: Msg) (state: State) =
    match msg with
    | FoldersLoaded fs      ->
        let selectedFolder =
            state.SelectedFolder
            |> Option.filter (fun path -> fs |> List.exists (fun root -> isPathWithinRoot root path))
        { state with Folders = fs; FolderTree = []; SelectedFolder = selectedFolder }
    | FolderTreeLoaded tree ->
        let selectedFolder =
            state.SelectedFolder
            |> Option.filter (fun path -> treeContainsPath path tree)
        { state with FolderTree = tree; SelectedFolder = selectedFolder }
    | TagsLoaded ts         -> { state with Tags = ts }
    | FolderRemoved f       ->
        let folders = state.Folders |> List.filter ((<>) f)
        let selectedFolder =
            state.SelectedFolder
            |> Option.filter (fun path -> folders |> List.exists (fun root -> isPathWithinRoot root path))
        { state with
            Folders = folders
            FolderTree = state.FolderTree |> List.filter (fun node -> node.Path <> f)
            SelectedFolder = selectedFolder }
    | FolderSelected path   ->
        { state with SelectedFolder = Some (normalizePath path) }
    | TagToggled tag    ->
        let selected =
            if state.SelectedTags |> List.contains tag
            then state.SelectedTags |> List.filter ((<>) tag)
            else state.SelectedTags @ [ tag ]
        { state with SelectedTags = selected }

let rec private folderNodeView (depth: int) (selectedPath: string option) (dispatch: Msg -> unit) (onRemoveRequested: string -> unit) (node: FolderNode) =
    let isSelected = selectedPath |> Option.exists (samePath node.Path)
    let foreground =
        SolidColorBrush(
            if isSelected then Theme.folderSelectedText
            elif depth = 0 then Theme.folderSelectedText
            else Theme.folderUnselectedText)
    let pathForeground =
        SolidColorBrush(
            if isSelected then Theme.folderSelectedPath
            else Theme.folderUnselectedPath)
    let selectionBackground : IBrush =
        if isSelected then SolidColorBrush(Theme.folderSelectedBg) :> IBrush
        else Brushes.Transparent :> IBrush
    let selectionBorder : IBrush =
        if isSelected then SolidColorBrush(Theme.folderSelectedBorder) :> IBrush
        else Brushes.Transparent :> IBrush
    let selectionBorderThickness =
        if isSelected then Avalonia.Thickness(3.0, 1.0, 1.0, 1.0)
        else Avalonia.Thickness(0.0)

    StackPanel.create [
        StackPanel.contextMenu (
            ContextMenu.create [
                ContextMenu.viewItems [
                    MenuItem.create [
                        MenuItem.header "Remove Folder"
                        MenuItem.onClick(
                            (fun _ -> onRemoveRequested node.Path),
                            SubPatchOptions.OnChangeOf node.Path)
                    ]
                ]
            ])
        StackPanel.children [
            yield Button.create [
                Button.background selectionBackground
                Button.borderBrush selectionBorder
                Button.borderThickness selectionBorderThickness
                Button.margin (Avalonia.Thickness(float (depth * 14), 2.0, 0.0, 4.0))
                Button.padding (Avalonia.Thickness(8.0, 4.0, 6.0, 4.0))
                Button.horizontalAlignment HorizontalAlignment.Stretch
                Button.horizontalContentAlignment HorizontalAlignment.Left
                Button.onClick(
                    (fun _ -> dispatch (FolderSelected node.Path)),
                    SubPatchOptions.OnChangeOf node.Path)
                Button.content (
                    StackPanel.create [
                        StackPanel.children [
                            TextBlock.create [
                                TextBlock.text node.Name
                                TextBlock.foreground foreground
                                TextBlock.fontSize Theme.FontSize.md
                                TextBlock.fontWeight (if depth = 0 then FontWeight.SemiBold else FontWeight.Normal)
                                TextBlock.textTrimming TextTrimming.CharacterEllipsis
                                TextBlock.tip node.Path
                            ]
                            TextBlock.create [
                                TextBlock.text node.Path
                                TextBlock.foreground pathForeground
                                TextBlock.fontSize Theme.FontSize.xs
                                TextBlock.textTrimming TextTrimming.CharacterEllipsis
                                TextBlock.tip node.Path
                            ]
                        ]
                    ])
            ] :> Avalonia.FuncUI.Types.IView
            for child in node.Children do
                yield folderNodeView (depth + 1) selectedPath dispatch onRemoveRequested child :> Avalonia.FuncUI.Types.IView
        ]
    ]

let private tagChip (tag: string) (count: int) (selected: bool) (dispatch: Msg -> unit) =
    Border.create [
        Border.cornerRadius 4.0
        Border.background (SolidColorBrush(if selected then Theme.accent else Theme.tagChipBg))
        Border.margin (Avalonia.Thickness(0.0, 2.0))
        Border.cursor Avalonia.Input.Cursor.Default
        Border.onPointerPressed(
            (fun _ -> dispatch (TagToggled tag)),
            SubPatchOptions.OnChangeOf tag)
        Border.child (
            StackPanel.create [
                StackPanel.orientation Orientation.Horizontal
                StackPanel.margin (Avalonia.Thickness(8.0, 4.0))
                StackPanel.children [
                    TextBlock.create [
                        TextBlock.text tag
                        TextBlock.foreground Brushes.White
                        TextBlock.fontSize Theme.FontSize.sm
                    ]
                    TextBlock.create [
                        TextBlock.text $" ({count})"
                        TextBlock.foreground (SolidColorBrush(Theme.textDim))
                        TextBlock.fontSize Theme.FontSize.xs
                    ]
                ]
            ])
    ]

let view (state: State) (dispatch: Msg -> unit) (onAddFolderRequested: unit -> unit) (onFolderRemoveRequested: string -> unit) =
    DockPanel.create [
        DockPanel.width 220.0
        DockPanel.background (SolidColorBrush(Theme.panelBg))
        DockPanel.children [
            ScrollViewer.create [
                ScrollViewer.content (
                    StackPanel.create [
                        StackPanel.margin (Avalonia.Thickness(8.0, 0.0))
                        StackPanel.children [
                            // Folder list
                            yield Grid.create [
                                Grid.columnDefinitions "*, Auto, Auto"
                                Grid.margin (Avalonia.Thickness(0.0, 8.0, 0.0, 4.0))
                                Grid.children [
                                    TextBlock.create [
                                        Grid.column 0
                                        TextBlock.text "Folders"
                                        TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                        TextBlock.fontSize Theme.FontSize.xs
                                        TextBlock.verticalAlignment Avalonia.Layout.VerticalAlignment.Center
                                    ]
                                    Button.create [
                                        Grid.column 1
                                        Button.content "-"
                                        Button.fontSize Theme.FontSize.lg
                                        Button.padding (Avalonia.Thickness(6.0, 0.0))
                                        Button.background Brushes.Transparent
                                        Button.borderThickness (Avalonia.Thickness(0.0))
                                        Button.foreground (SolidColorBrush(if state.SelectedFolder.IsSome then Theme.textMuted else Theme.textMuted |> fun c -> Avalonia.Media.Color.FromArgb(80uy, c.R, c.G, c.B)))
                                        Button.tip "Remove Selected Folder"
                                        Button.onClick(
                                            (fun _ ->
                                                match state.SelectedFolder with
                                                | Some path -> onFolderRemoveRequested path
                                                | None -> ()),
                                            SubPatchOptions.OnChangeOf state.SelectedFolder)
                                    ]
                                    Button.create [
                                        Grid.column 2
                                        Button.content "+"
                                        Button.fontSize Theme.FontSize.lg
                                        Button.padding (Avalonia.Thickness(6.0, 0.0))
                                        Button.background Brushes.Transparent
                                        Button.borderThickness (Avalonia.Thickness(0.0))
                                        Button.foreground (SolidColorBrush(Theme.textMuted))
                                        Button.tip "Add Folder"
                                        Button.onClick (fun _ -> onAddFolderRequested())
                                    ]
                                ]
                            ] :> Avalonia.FuncUI.Types.IView
                            if state.FolderTree.IsEmpty then
                                for folder in state.Folders do
                                    yield folderNodeView 0 state.SelectedFolder dispatch onFolderRemoveRequested { Name = displayName folder; Path = folder; Children = [] }
                                          :> Avalonia.FuncUI.Types.IView
                            else
                                for node in state.FolderTree do
                                    yield folderNodeView 0 state.SelectedFolder dispatch onFolderRemoveRequested node :> Avalonia.FuncUI.Types.IView
                            // Tag list
                            if not state.Tags.IsEmpty then
                                yield TextBlock.create [
                                    TextBlock.text "TAGS"
                                    TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                    TextBlock.fontSize Theme.FontSize.xs
                                    TextBlock.margin (Avalonia.Thickness(0.0, 16.0, 0.0, 4.0))
                                ] :> Avalonia.FuncUI.Types.IView
                            for tag, count in state.Tags do
                                yield tagChip tag count (state.SelectedTags |> List.contains tag) dispatch
                                      :> Avalonia.FuncUI.Types.IView
                        ]
                    ])
            ]
        ]
    ]
