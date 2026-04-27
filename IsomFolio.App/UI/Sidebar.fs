module IsomFolio.UI.Sidebar

open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
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
        let path = normalizePath path
        let selected =
            match state.SelectedFolder with
            | Some current when samePath current path -> None
            | _ -> Some path
        { state with SelectedFolder = selected }
    | TagToggled tag    ->
        let selected =
            if state.SelectedTags |> List.contains tag
            then state.SelectedTags |> List.filter ((<>) tag)
            else state.SelectedTags @ [ tag ]
        { state with SelectedTags = selected }

let rec private folderNodeView (depth: int) (selectedPath: string option) (dispatch: Msg -> unit) (node: FolderNode) =
    let isSelected = selectedPath |> Option.exists (samePath node.Path)
    let foreground =
        SolidColorBrush(
            if isSelected then Color.Parse("#FFFFFF")
            elif depth = 0 then Color.Parse("#FFFFFF")
            else Color.Parse("#CFCFCF"))
    let pathForeground =
        SolidColorBrush(
            if isSelected then Color.Parse("#D6ECFF")
            else Color.Parse("#7F7F7F"))
    let selectionBackground : IBrush =
        if isSelected then SolidColorBrush(Color.Parse("#0B3D62")) :> IBrush
        else Brushes.Transparent :> IBrush
    let selectionBorder : IBrush =
        if isSelected then SolidColorBrush(Color.Parse("#8FD3FF")) :> IBrush
        else Brushes.Transparent :> IBrush
    let selectionBorderThickness =
        if isSelected then Avalonia.Thickness(3.0, 1.0, 1.0, 1.0)
        else Avalonia.Thickness(0.0)

    StackPanel.create [
        StackPanel.children [
            yield Button.create [
                Button.background selectionBackground
                Button.borderBrush selectionBorder
                Button.borderThickness selectionBorderThickness
                Button.margin (Avalonia.Thickness(float (depth * 14), 2.0, 0.0, 4.0))
                Button.padding (Avalonia.Thickness(8.0, 4.0, 6.0, 4.0))
                Button.horizontalAlignment HorizontalAlignment.Stretch
                Button.horizontalContentAlignment HorizontalAlignment.Left
                Button.onClick (fun _ -> dispatch (FolderSelected node.Path))
                Button.content (
                    StackPanel.create [
                        StackPanel.children [
                            TextBlock.create [
                                TextBlock.text node.Name
                                TextBlock.foreground foreground
                                TextBlock.fontSize 13.0
                                TextBlock.fontWeight (if depth = 0 then FontWeight.SemiBold else FontWeight.Normal)
                                TextBlock.textTrimming Avalonia.Media.TextTrimming.CharacterEllipsis
                                TextBlock.tip node.Path
                            ]
                            TextBlock.create [
                                TextBlock.text node.Path
                                TextBlock.foreground pathForeground
                                TextBlock.fontSize 11.0
                                TextBlock.textTrimming Avalonia.Media.TextTrimming.CharacterEllipsis
                                TextBlock.tip node.Path
                            ]
                        ]
                    ])
            ] :> Avalonia.FuncUI.Types.IView
            for child in node.Children do
                yield folderNodeView (depth + 1) selectedPath dispatch child :> Avalonia.FuncUI.Types.IView
        ]
    ]

let private tagChip (tag: string) (count: int) (selected: bool) (dispatch: Msg -> unit) =
    Border.create [
        Border.cornerRadius 4.0
        Border.background (if selected then "#0078D4" else "#333333")
        Border.margin (Avalonia.Thickness(0.0, 2.0))
        Border.cursor Avalonia.Input.Cursor.Default
        Border.onPointerPressed (fun _ -> dispatch (TagToggled tag))
        Border.child (
            StackPanel.create [
                StackPanel.orientation Orientation.Horizontal
                StackPanel.margin (Avalonia.Thickness(8.0, 4.0))
                StackPanel.children [
                    TextBlock.create [
                        TextBlock.text tag
                        TextBlock.foreground Brushes.White
                        TextBlock.fontSize 12.0
                    ]
                    TextBlock.create [
                        TextBlock.text $" ({count})"
                        TextBlock.foreground (SolidColorBrush(Color.Parse("#AAAAAA")))
                        TextBlock.fontSize 11.0
                    ]
                ]
            ])
    ]

let view (state: State) (dispatch: Msg -> unit) (onAddFolderRequested: unit -> unit) =
    DockPanel.create [
        DockPanel.width 220.0
        DockPanel.background (SolidColorBrush(Color.Parse("#1E1E1E")))
        DockPanel.children [
            Button.create [
                Button.dock Dock.Top
                Button.content "Add Folder…"
                Button.horizontalAlignment HorizontalAlignment.Stretch
                Button.margin (Avalonia.Thickness(8.0))
                Button.onClick (fun _ -> onAddFolderRequested())
            ]
            ScrollViewer.create [
                ScrollViewer.content (
                    StackPanel.create [
                        StackPanel.margin (Avalonia.Thickness(8.0, 0.0))
                        StackPanel.children [
                            // Folder list
                            yield TextBlock.create [
                                TextBlock.text "FOLDERS"
                                TextBlock.foreground (SolidColorBrush(Color.Parse("#888888")))
                                TextBlock.fontSize 11.0
                                TextBlock.margin (Avalonia.Thickness(0.0, 8.0, 0.0, 4.0))
                            ] :> Avalonia.FuncUI.Types.IView
                            if state.FolderTree.IsEmpty then
                                for folder in state.Folders do
                                    yield folderNodeView 0 state.SelectedFolder dispatch { Name = displayName folder; Path = folder; Children = [] }
                                          :> Avalonia.FuncUI.Types.IView
                            else
                                for node in state.FolderTree do
                                    yield folderNodeView 0 state.SelectedFolder dispatch node :> Avalonia.FuncUI.Types.IView
                            // Tag list
                            if not state.Tags.IsEmpty then
                                yield TextBlock.create [
                                    TextBlock.text "TAGS"
                                    TextBlock.foreground (SolidColorBrush(Color.Parse("#888888")))
                                    TextBlock.fontSize 11.0
                                    TextBlock.margin (Avalonia.Thickness(0.0, 16.0, 0.0, 4.0))
                                ] :> Avalonia.FuncUI.Types.IView
                            for (tag, count) in state.Tags do
                                yield tagChip tag count (state.SelectedTags |> List.contains tag) dispatch
                                      :> Avalonia.FuncUI.Types.IView
                        ]
                    ])
            ]
        ]
    ]
