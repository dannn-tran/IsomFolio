module IsomFolio.UI.Sidebar
#nowarn "44"

open Avalonia.FuncUI.Builder
open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Input
open Avalonia.Layout
open Avalonia.Media
open IsomFolio.Core.PathUtils
open IsomFolio.Core.Indexing.FolderTree
open IsomFolio.Core.Models
open IsomFolio.UI.ContextMenuExt

type State = {
    Folders               : string list
    FolderTree            : FolderNode list
    Tags                  : (string * int) list   // (name, count)
    SelectedTags          : string list
    SelectedFolder        : string option
    CollapsedFolders      : Set<string>
    Albums                : Album list
    SelectedAlbumId       : AlbumId option
    FolderCounts          : Map<string, int>
    FolderSearchRecursive : bool
}

type Msg =
    | FolderRemoved        of string
    | FolderSelected       of string
    | FolderToggled        of string
    | TagToggled           of string
    | FoldersLoaded        of string list
    | FolderTreeLoaded     of FolderNode list
    | TagsLoaded           of (string * int) list
    | AlbumsLoaded         of Album list
    | AlbumSelected        of AlbumId
    | AlbumDeselected
    | FolderDeselected
    | AlbumCreateRequested
    | AlbumRenameRequested          of AlbumId
    | AlbumDeleteRequested          of AlbumId
    | AlbumEditCriteriaRequested    of AlbumId
    | FolderCountsLoaded            of Map<string, int>
    | FolderSearchRecursiveToggled
    | FileDroppedToAlbum            of FileId * AlbumId

let init () = {
    Folders = []; FolderTree = []; Tags = []; SelectedTags = []
    SelectedFolder = None; CollapsedFolders = Set.empty
    Albums = []; SelectedAlbumId = None; FolderCounts = Map.empty
    FolderSearchRecursive = true
}

let private isPathWithinRoot (root: string) (path: string) = isWithinSubtree root path

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
        let folders = state.Folders |> List.filter (fun p -> not (isWithinSubtree f p))
        let selectedFolder =
            state.SelectedFolder
            |> Option.filter (fun path -> folders |> List.exists (fun root -> isPathWithinRoot root path))
        { state with
            Folders = folders
            FolderTree = state.FolderTree |> List.filter (fun node -> node.Path <> f)
            SelectedFolder = selectedFolder }
    | FolderDeselected        -> { state with SelectedFolder = None }
    | FolderSelected path   ->
        { state with SelectedFolder = Some (normalizePath path); SelectedAlbumId = None }
    | FolderToggled path ->
        let collapsed =
            if state.CollapsedFolders |> Set.contains path
            then state.CollapsedFolders |> Set.remove path
            else state.CollapsedFolders |> Set.add path
        { state with CollapsedFolders = collapsed }
    | TagToggled tag    ->
        let selected =
            if state.SelectedTags |> List.contains tag
            then state.SelectedTags |> List.filter ((<>) tag)
            else state.SelectedTags @ [ tag ]
        { state with SelectedTags = selected }
    | AlbumsLoaded albums              -> { state with Albums = albums }
    | AlbumSelected id                 -> { state with SelectedAlbumId = Some id; SelectedFolder = None }
    | AlbumDeselected                  -> { state with SelectedAlbumId = None }
    | FolderCountsLoaded counts        -> { state with FolderCounts = counts }
    | FolderSearchRecursiveToggled     -> { state with FolderSearchRecursive = not state.FolderSearchRecursive }
    | FileDroppedToAlbum _             -> state
    | AlbumCreateRequested
    | AlbumRenameRequested _
    | AlbumDeleteRequested _
    | AlbumEditCriteriaRequested _     -> state

let private hasPendingIn (nodePath: string) (pendingFolders: Set<string>) =
    let normalizedPath = normalizePath nodePath
    pendingFolders |> Set.exists (fun p -> isWithinSubtree normalizedPath p)

let rec private folderNodeView (depth: int) (selectedPath: string option) (pendingFolders: Set<string>) (collapsedFolders: Set<string>) (folderCounts: Map<string, int>) (dispatch: Msg -> unit) (onRemoveRequested: string -> unit) (onResyncRequested: string -> unit) (node: FolderNode) =
    let isSelected = selectedPath |> Option.exists (samePath node.Path)
    let isCollapsed = collapsedFolders |> Set.contains node.Path
    let foreground =
        SolidColorBrush(
            if isSelected then Theme.folderSelectedText
            elif depth = 0 then Theme.folderSelectedText
            else Theme.folderUnselectedText)
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
        XStackPanel.contextMenu (
            XContextMenu.create [
                XContextMenu.viewItems [
                    XMenuItem.create [
                        XMenuItem.header "Remove Folder"
                        XMenuItem.onClick (fun _ -> onRemoveRequested node.Path)
                    ]
                ]
            ])
        StackPanel.children [
            yield DockPanel.create [
                DockPanel.margin (Avalonia.Thickness(float (depth * 14), 2.0, 0.0, 2.0))
                DockPanel.children [
                    Button.create [
                        Button.dock Dock.Right
                        Button.isVisible (hasPendingIn node.Path pendingFolders)
                        Button.content "↻"
                        Button.fontSize Theme.FontSize.lg
                        Button.foreground (SolidColorBrush(Theme.pendingAmber))
                        Button.background Brushes.Transparent
                        Button.borderThickness (Avalonia.Thickness(0.0))
                        Button.padding (Avalonia.Thickness(6.0, 0.0))
                        Button.tip "Changes detected — click to resync"
                        Button.onClick(
                            (fun _ -> onResyncRequested node.Path),
                            SubPatchOptions.OnChangeOf node.Path)
                    ] :> Avalonia.FuncUI.Types.IView
                    Button.create [
                        Button.dock Dock.Left
                        Button.isVisible (not node.Children.IsEmpty)
                        Button.content (if isCollapsed then "▸" else "▾")
                        Button.fontSize Theme.FontSize.xs
                        Button.foreground (SolidColorBrush(Theme.textMuted))
                        Button.background Brushes.Transparent
                        Button.borderThickness (Avalonia.Thickness(0.0))
                        Button.padding (Avalonia.Thickness(4.0, 0.0, 2.0, 0.0))
                        Button.tip (if isCollapsed then "Expand" else "Collapse")
                        Button.onClick(
                            (fun _ -> dispatch (FolderToggled node.Path)),
                            SubPatchOptions.OnChangeOf node.Path)
                    ] :> Avalonia.FuncUI.Types.IView
                    Button.create [
                        Button.background selectionBackground
                        Button.borderBrush selectionBorder
                        Button.borderThickness selectionBorderThickness
                        Button.padding (Avalonia.Thickness(8.0, 4.0, 6.0, 4.0))
                        Button.horizontalAlignment HorizontalAlignment.Stretch
                        Button.horizontalContentAlignment HorizontalAlignment.Left
                        Button.onClick(
                            (fun _ -> dispatch (FolderSelected node.Path)),
                            SubPatchOptions.OnChangeOf node.Path)
                        Button.content (
                            let count = folderCounts |> Map.tryFind node.Path |> Option.defaultValue 0
                            TextBlock.create [
                                TextBlock.text (if count > 0 then $"{node.Name}  ({count})" else node.Name)
                                TextBlock.foreground foreground
                                TextBlock.fontSize Theme.FontSize.md
                                TextBlock.fontWeight (if depth = 0 then FontWeight.SemiBold else FontWeight.Normal)
                                TextBlock.textTrimming TextTrimming.CharacterEllipsis
                                if depth = 0 then TextBlock.tip node.Path
                            ])
                    ] :> Avalonia.FuncUI.Types.IView
                ]
            ] :> Avalonia.FuncUI.Types.IView
            if not isCollapsed then
                for child in node.Children do
                    yield folderNodeView (depth + 1) selectedPath pendingFolders collapsedFolders folderCounts dispatch onRemoveRequested onResyncRequested child :> Avalonia.FuncUI.Types.IView
        ]
    ]

let private albumRow (album: Album) (selectedId: AlbumId option) (dispatch: Msg -> unit) =
    let isSelected = selectedId = Some album.Id
    let isSmart = match album.Kind with Smart _ -> true | Manual -> false
    let selBg : IBrush =
        if isSelected then SolidColorBrush(Theme.folderSelectedBg) :> IBrush
        else Brushes.Transparent :> IBrush
    let selBorder : IBrush =
        if isSelected then SolidColorBrush(Theme.folderSelectedBorder) :> IBrush
        else Brushes.Transparent :> IBrush
    let menuItems = [
        yield XMenuItem.create [
            XMenuItem.header "Rename…"
            XMenuItem.onClick (fun _ -> dispatch (AlbumRenameRequested album.Id))
        ]
        if isSmart then
            yield XMenuItem.create [
                XMenuItem.header "Edit Criteria…"
                XMenuItem.onClick (fun _ -> dispatch (AlbumEditCriteriaRequested album.Id))
            ]
        yield XMenuItem.create [
            XMenuItem.header "Delete"
            XMenuItem.onClick (fun _ -> dispatch (AlbumDeleteRequested album.Id))
        ]
    ]
    StackPanel.create [
        XStackPanel.contextMenu (
            XContextMenu.create [
                XContextMenu.viewItems menuItems
            ])
        StackPanel.children [
            Button.create [
                yield Button.background selBg
                yield Button.borderBrush selBorder
                yield Button.borderThickness (if isSelected then Avalonia.Thickness(3.0, 1.0, 1.0, 1.0) else Avalonia.Thickness(0.0))
                yield Button.padding (Avalonia.Thickness(8.0, 4.0, 6.0, 4.0))
                yield Button.horizontalAlignment HorizontalAlignment.Stretch
                yield Button.horizontalContentAlignment HorizontalAlignment.Left
                yield Button.onClick(
                    (fun _ ->
                        if isSelected then dispatch AlbumDeselected
                        else dispatch (AlbumSelected album.Id)),
                    SubPatchOptions.OnChangeOf album.Id)
                yield Button.content (
                    TextBlock.create [
                        TextBlock.text (if isSmart then $"★ {album.Name}" else album.Name)
                        TextBlock.foreground (SolidColorBrush(if isSelected then Theme.folderSelectedText else Theme.folderUnselectedText))
                        TextBlock.fontSize Theme.FontSize.md
                        TextBlock.textTrimming TextTrimming.CharacterEllipsis
                        if isSmart then TextBlock.tip "Smart Album — defined by search criteria"
                    ])
                if not isSmart then
                    yield AttrBuilder<Avalonia.Controls.Button>.CreateProperty<bool>(DragDrop.AllowDropProperty, true, ValueNone)
                    yield AttrBuilder<Avalonia.Controls.Button>.CreateSubscription<DragEventArgs>(
                        DragDrop.DragOverEvent,
                        fun args ->
                            if args.Data.Contains("IsomFolio.FileIds") then
                                args.DragEffects <- DragDropEffects.Copy
                            else
                                args.DragEffects <- DragDropEffects.None)
                    yield AttrBuilder<Avalonia.Controls.Button>.CreateSubscription<DragEventArgs>(
                        DragDrop.DropEvent,
                        fun (args: DragEventArgs) ->
                            match args.Data.Get("IsomFolio.FileIds") with
                            | :? string as csv ->
                                for fileId in csv.Split(',') do
                                    dispatch (FileDroppedToAlbum(fileId, album.Id))
                            | _ -> ())
            ]
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

let view (state: State) (dispatch: Msg -> unit) (pendingFolders: Set<string>) (onAddFolderRequested: unit -> unit) (onFolderRemoveRequested: string -> unit) (onResyncRequested: string -> unit) (catalogName: string option) (onNewCatalogRequested: unit -> unit) (onOpenCatalogRequested: unit -> unit) =
    DockPanel.create [
        DockPanel.width 220.0
        DockPanel.background (SolidColorBrush(Theme.panelBg))
        DockPanel.children [
            Border.create [
                Border.dock Dock.Bottom
                Border.borderThickness (Avalonia.Thickness(0.0, 1.0, 0.0, 0.0))
                Border.borderBrush (SolidColorBrush(Color.FromArgb(60uy, 136uy, 136uy, 136uy)))
                Border.padding (Avalonia.Thickness(8.0, 6.0))
                Border.child (
                    StackPanel.create [
                        StackPanel.spacing 4.0
                        StackPanel.children [
                            match catalogName with
                            | Some name ->
                                yield TextBlock.create [
                                    TextBlock.text name
                                    TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                    TextBlock.fontSize Theme.FontSize.xs
                                    TextBlock.textTrimming TextTrimming.CharacterEllipsis
                                    TextBlock.tip name
                                ] :> Avalonia.FuncUI.Types.IView
                            | None -> ()
                            yield StackPanel.create [
                                StackPanel.orientation Orientation.Horizontal
                                StackPanel.spacing 2.0
                                StackPanel.children [
                                    Button.create [
                                        Button.content "New Catalog…"
                                        Button.fontSize Theme.FontSize.xs
                                        Button.foreground (SolidColorBrush(Theme.textMuted))
                                        Button.background Brushes.Transparent
                                        Button.borderThickness (Avalonia.Thickness(0.0))
                                        Button.padding (Avalonia.Thickness(0.0))
                                        Button.onClick (fun _ -> onNewCatalogRequested())
                                    ]
                                    TextBlock.create [
                                        TextBlock.text "·"
                                        TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                        TextBlock.fontSize Theme.FontSize.xs
                                        TextBlock.verticalAlignment Avalonia.Layout.VerticalAlignment.Center
                                    ]
                                    Button.create [
                                        Button.content "Open Catalog…"
                                        Button.fontSize Theme.FontSize.xs
                                        Button.foreground (SolidColorBrush(Theme.textMuted))
                                        Button.background Brushes.Transparent
                                        Button.borderThickness (Avalonia.Thickness(0.0))
                                        Button.padding (Avalonia.Thickness(0.0))
                                        Button.onClick (fun _ -> onOpenCatalogRequested())
                                    ]
                                ]
                            ] :> Avalonia.FuncUI.Types.IView
                        ]
                    ])
            ] :> Avalonia.FuncUI.Types.IView
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
                                    yield folderNodeView 0 state.SelectedFolder pendingFolders state.CollapsedFolders state.FolderCounts dispatch onFolderRemoveRequested onResyncRequested { Name = displayName folder; Path = folder; Children = [] }
                                          :> Avalonia.FuncUI.Types.IView
                            else
                                for node in state.FolderTree do
                                    yield folderNodeView 0 state.SelectedFolder pendingFolders state.CollapsedFolders state.FolderCounts dispatch onFolderRemoveRequested onResyncRequested node :> Avalonia.FuncUI.Types.IView
                            if state.SelectedFolder.IsSome then
                                yield StackPanel.create [
                                    StackPanel.orientation Orientation.Horizontal
                                    StackPanel.margin (Avalonia.Thickness(4.0, 4.0, 0.0, 0.0))
                                    StackPanel.spacing 6.0
                                    StackPanel.children [
                                        CheckBox.create [
                                            CheckBox.isChecked state.FolderSearchRecursive
                                            CheckBox.onClick(
                                                (fun _ -> dispatch FolderSearchRecursiveToggled),
                                                SubPatchOptions.OnChangeOf state.FolderSearchRecursive)
                                        ]
                                        TextBlock.create [
                                            TextBlock.text "Include subfolders"
                                            TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                            TextBlock.fontSize Theme.FontSize.xs
                                            TextBlock.verticalAlignment Avalonia.Layout.VerticalAlignment.Center
                                        ]
                                    ]
                                ] :> Avalonia.FuncUI.Types.IView
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
                            // Album list
                            yield Grid.create [
                                Grid.columnDefinitions "*, Auto"
                                Grid.margin (Avalonia.Thickness(0.0, 16.0, 0.0, 4.0))
                                Grid.children [
                                    TextBlock.create [
                                        Grid.column 0
                                        TextBlock.text "ALBUMS"
                                        TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                        TextBlock.fontSize Theme.FontSize.xs
                                        TextBlock.verticalAlignment Avalonia.Layout.VerticalAlignment.Center
                                    ]
                                    Button.create [
                                        Grid.column 1
                                        Button.content "+"
                                        Button.fontSize Theme.FontSize.lg
                                        Button.padding (Avalonia.Thickness(6.0, 0.0))
                                        Button.background Brushes.Transparent
                                        Button.borderThickness (Avalonia.Thickness(0.0))
                                        Button.foreground (SolidColorBrush(Theme.textMuted))
                                        Button.tip "New Album"
                                        Button.onClick (fun _ -> dispatch AlbumCreateRequested)
                                    ]
                                ]
                            ] :> Avalonia.FuncUI.Types.IView
                            for album in state.Albums do
                                yield albumRow album state.SelectedAlbumId dispatch :> Avalonia.FuncUI.Types.IView
                        ]
                    ])
            ]
        ]
    ]
