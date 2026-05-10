module IsomFolio.UI.GridView
#nowarn "44"

open System.Runtime.InteropServices
open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Input
open Avalonia.Layout
open Avalonia.Media
open Avalonia.Media.Imaging
open IsomFolio.Core.Models
open IsomFolio.Core.FileIndex
open IsomFolio.UI.ContextMenuExt

type TileModel = {
    File      : AssetFile
    Thumbnail : ThumbnailState
}

type SelectionModifier = Plain | RangeExtend | Toggle

type State = {
    Tiles          : TileModel list
    TileSize       : TileSize
    SelectedIds    : Set<FileId>
    AnchorId       : FileId option   // last non-shift click; used for range extension and detail panel
    Albums         : Album list      // manual albums for "Add to Album" context menu
    CurrentAlbumId : AlbumId option  // set when viewing a manual album
}

type NavDirection = Left | Right | Up | Down

type Msg =
    | TilesLoaded                  of AssetFile list
    | ThumbnailUpdated             of FileId * ThumbnailState
    | TileSizeChanged              of TileSize
    | TileClicked                  of FileId * SelectionModifier
    | NavigateTo                   of NavDirection * int
    | RemoveOrphanedFileRequested  of FileId
    | OpenExternally               of FileId
    | RevealInExplorer             of FileId
    | EnterLoupe                   of FileId
    | AlbumsUpdated                of Album list
    | CurrentAlbumChanged          of AlbumId option
    | AddToAlbum                   of FileId * AlbumId
    | RemoveFromAlbum              of FileId * AlbumId

let init () = {
    Tiles = []; TileSize = Medium
    SelectedIds = Set.empty; AnchorId = None
    Albums = []; CurrentAlbumId = None
}

let update (msg: Msg) (state: State) =
    match msg with
    | TilesLoaded files ->
        let files = files |> List.distinctBy (fun f -> f.Id)
        let existingStates =
            state.Tiles
            |> List.map (fun tile -> tile.File.Id, tile.Thumbnail)
            |> Map.ofList
        let tiles =
            files
            |> List.map (fun f ->
                { File = f
                  Thumbnail = existingStates |> Map.tryFind f.Id |> Option.defaultValue NotRequested })
        let newSelectedIds = state.SelectedIds |> Set.filter (fun id -> files |> List.exists (fun f -> f.Id = id))
        let newAnchorId    = state.AnchorId    |> Option.filter (fun id -> files |> List.exists (fun f -> f.Id = id))
        { state with Tiles = tiles; SelectedIds = newSelectedIds; AnchorId = newAnchorId }
    | ThumbnailUpdated(fileId, thumbState) ->
        let tiles =
            state.Tiles |> List.map (fun t ->
                if t.File.Id = fileId then { t with Thumbnail = thumbState } else t)
        { state with Tiles = tiles }
    | TileSizeChanged ts -> { state with TileSize = ts }
    | TileClicked (id, Plain) ->
        { state with SelectedIds = Set.singleton id; AnchorId = Some id }
    | TileClicked (id, Toggle) ->
        let newIds =
            if state.SelectedIds.Contains id then state.SelectedIds.Remove id
            else state.SelectedIds.Add id
        { state with SelectedIds = newIds; AnchorId = Some id }
    | TileClicked (id, RangeExtend) ->
        let anchorIdx =
            state.AnchorId
            |> Option.bind (fun aid -> state.Tiles |> List.tryFindIndex (fun t -> t.File.Id = aid))
            |> Option.defaultValue 0
        let clickedIdx =
            state.Tiles |> List.tryFindIndex (fun t -> t.File.Id = id)
            |> Option.defaultValue 0
        let lo, hi = min anchorIdx clickedIdx, max anchorIdx clickedIdx
        let rangeIds = state.Tiles.[lo..hi] |> List.map (fun t -> t.File.Id) |> Set.ofList
        { state with SelectedIds = rangeIds }
    | RemoveOrphanedFileRequested _
    | OpenExternally _
    | RevealInExplorer _
    | EnterLoupe _
    | AddToAlbum _
    | RemoveFromAlbum _                -> state
    | AlbumsUpdated albums             -> { state with Albums = albums }
    | CurrentAlbumChanged id           -> { state with CurrentAlbumId = id }
    | NavigateTo (dir, rowSize) ->
        if state.Tiles.IsEmpty then state
        else
            let currentIdx =
                state.AnchorId
                |> Option.bind (fun id -> state.Tiles |> List.tryFindIndex (fun t -> t.File.Id = id))
                |> Option.defaultValue 0
            let newIdx =
                match dir with
                | Left  -> max 0 (currentIdx - 1)
                | Right -> min (state.Tiles.Length - 1) (currentIdx + 1)
                | Up    ->
                    let i = currentIdx - rowSize
                    if i < 0 then currentIdx else i
                | Down  ->
                    let i = currentIdx + rowSize
                    if i >= state.Tiles.Length then currentIdx else i
            let newId = state.Tiles.[newIdx].File.Id
            { state with SelectedIds = Set.singleton newId; AnchorId = Some newId }

let mutable private dragStartPoint = Avalonia.Point()
let mutable private dragCandidateFileId: FileId option = None

let private bitmapCache = System.Collections.Generic.Dictionary<string, Bitmap>()

let clearBitmapCache () =
    for kv in bitmapCache do kv.Value.Dispose()
    bitmapCache.Clear()

let private tryLoadBitmap (path: string) : Bitmap option =
    match bitmapCache.TryGetValue(path) with
    | true, bmp -> Some bmp
    | _ ->
        try
            let bmp = new Bitmap(path)
            bitmapCache[path] <- bmp
            Some bmp
        with _ -> None

let private revealLabel =
    if RuntimeInformation.IsOSPlatform(OSPlatform.OSX) then "Reveal in Finder"
    elif RuntimeInformation.IsOSPlatform(OSPlatform.Windows) then "Reveal in Explorer"
    else "Reveal in File Manager"

let private tile (model: TileModel) (sizePx: int) (selected: bool) (albums: Album list) (currentAlbumId: AlbumId option) (dispatch: Msg -> unit) =
    let px = float sizePx
    let menuItems =
        [
            XMenuItem.create [
                XMenuItem.header "Open"
                XMenuItem.onClick (fun _ -> dispatch (OpenExternally model.File.Id))
            ]
            XMenuItem.create [
                XMenuItem.header revealLabel
                XMenuItem.onClick (fun _ -> dispatch (RevealInExplorer model.File.Id))
            ]
            if model.File.IsOrphaned then
                XMenuItem.create [
                    XMenuItem.header "Remove from catalog"
                    XMenuItem.onClick (fun _ -> dispatch (RemoveOrphanedFileRequested model.File.Id))
                ]
            if not albums.IsEmpty then
                XMenuItem.create [
                    XMenuItem.header "Add to Album"
                    XMenuItem.subItems [
                        for album in albums do
                            yield XMenuItem.create [
                                XMenuItem.header album.Name
                                XMenuItem.onClick (fun _ -> dispatch (AddToAlbum (model.File.Id, album.Id)))
                            ]
                    ]
                ]
            match currentAlbumId with
            | Some albumId ->
                XMenuItem.create [
                    XMenuItem.header "Remove from Album"
                    XMenuItem.onClick (fun _ -> dispatch (RemoveFromAlbum (model.File.Id, albumId)))
                ]
            | None -> ()
        ]
    Border.create [
        Border.width px
        Border.height (px + 24.0)
        Border.margin (Avalonia.Thickness(4.0))
        Border.cornerRadius 4.0
        Border.background (if selected then SolidColorBrush(Theme.accent) else SolidColorBrush(Theme.tileBg))
        XBorder.contextMenu (
            XContextMenu.create [
                XContextMenu.viewItems menuItems
            ])
        Border.onTapped(
            (fun e ->
                let selMod =
                    if e.KeyModifiers.HasFlag(KeyModifiers.Shift) then RangeExtend
                    elif e.KeyModifiers.HasFlag(KeyModifiers.Control) || e.KeyModifiers.HasFlag(KeyModifiers.Meta) then Toggle
                    else Plain
                dispatch (TileClicked(model.File.Id, selMod))
                match e.Source with
                | :? Avalonia.Visual as v ->
                    let sv = Avalonia.VisualTree.VisualExtensions.FindAncestorOfType<ScrollViewer>(v, true)
                    if not (isNull sv) then sv.Focus() |> ignore
                | _ -> ()),
            SubPatchOptions.OnChangeOf model.File.Id)
        Border.onDoubleTapped(
            (fun _ -> dispatch (EnterLoupe model.File.Id)),
            SubPatchOptions.OnChangeOf model.File.Id)
        Border.onPointerPressed(
            (fun e ->
                dragStartPoint <- e.GetCurrentPoint(Unchecked.defaultof<Avalonia.Visual>).Position
                dragCandidateFileId <- Some model.File.Id
                match e.Source with
                | :? Avalonia.Input.IInputElement as src -> e.Pointer.Capture(src)
                | _ -> ()),
            SubPatchOptions.OnChangeOf model.File.Id)
        Border.onPointerMoved(
            (fun e ->
                let point = e.GetCurrentPoint(Unchecked.defaultof<Avalonia.Visual>)
                match dragCandidateFileId with
                | Some fileId when point.Properties.IsLeftButtonPressed ->
                    let pos = point.Position
                    if abs(pos.X - dragStartPoint.X) > 8.0 || abs(pos.Y - dragStartPoint.Y) > 8.0 then
                        dragCandidateFileId <- None
                        let data = DataObject()
                        data.Set("IsomFolio.FileId", fileId :> obj)
                        DragDrop.DoDragDrop(e, data, DragDropEffects.Copy) |> ignore
                | _ -> ()),
            SubPatchOptions.OnChangeOf model.File.Id)
        Border.onPointerReleased(
            (fun _ -> dragCandidateFileId <- None),
            SubPatchOptions.OnChangeOf model.File.Id)
        Border.child (
            Grid.create [
                Grid.children [
                    DockPanel.create [
                        DockPanel.children [
                            TextBlock.create [
                                TextBlock.dock Dock.Bottom
                                TextBlock.text model.File.Name
                                TextBlock.fontSize Theme.FontSize.xs
                                TextBlock.foreground Brushes.White
                                TextBlock.textTrimming Avalonia.Media.TextTrimming.CharacterEllipsis
                                TextBlock.margin (Avalonia.Thickness(4.0, 2.0))
                                TextBlock.horizontalAlignment HorizontalAlignment.Center
                            ]
                            match model.Thumbnail with
                            | Ready path ->
                                match tryLoadBitmap path with
                                | Some bmp ->
                                    Image.create [
                                        Image.source bmp
                                        Image.stretch Stretch.Uniform
                                        Image.horizontalAlignment HorizontalAlignment.Center
                                        Image.verticalAlignment VerticalAlignment.Center
                                    ]
                                | None ->
                                    TextBlock.create [
                                        TextBlock.text "⚠"
                                        TextBlock.horizontalAlignment HorizontalAlignment.Center
                                        TextBlock.verticalAlignment VerticalAlignment.Center
                                    ]
                            | Pending ->
                                ProgressBar.create [
                                    ProgressBar.isIndeterminate true
                                    ProgressBar.height 4.0
                                    ProgressBar.verticalAlignment VerticalAlignment.Center
                                    ProgressBar.margin (Avalonia.Thickness(8.0))
                                ]
                            | Failed _ ->
                                TextBlock.create [
                                    TextBlock.text "⚠"
                                    TextBlock.fontSize 24.0
                                    TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                    TextBlock.horizontalAlignment HorizontalAlignment.Center
                                    TextBlock.verticalAlignment VerticalAlignment.Center
                                ]
                            | NotRequested ->
                                Border.create [
                                    Border.background (SolidColorBrush(Theme.surfaceBg))
                                    Border.horizontalAlignment HorizontalAlignment.Stretch
                                    Border.verticalAlignment VerticalAlignment.Stretch
                                ]
                        ]
                    ] :> Avalonia.FuncUI.Types.IView
                    if model.File.IsOrphaned then
                        Border.create [
                            Border.background (SolidColorBrush(Color.FromArgb(200uy, 30uy, 30uy, 30uy)))
                            Border.cornerRadius 3.0
                            Border.padding (Avalonia.Thickness(4.0, 2.0))
                            Border.margin (Avalonia.Thickness(4.0))
                            Border.horizontalAlignment HorizontalAlignment.Right
                            Border.verticalAlignment VerticalAlignment.Top
                            Border.child (
                                TextBlock.create [
                                    TextBlock.text "?"
                                    TextBlock.foreground (SolidColorBrush(Theme.textDim))
                                    TextBlock.fontSize Theme.FontSize.xs
                                ])
                        ] :> Avalonia.FuncUI.Types.IView
                ]
            ])
    ]

let private tileSizeButton (label: string) (ts: TileSize) (current: TileSize) (dispatch: Msg -> unit) =
    Button.create [
        Button.content label
        Button.margin (Avalonia.Thickness(2.0, 0.0))
        Button.background (SolidColorBrush(if ts = current then Theme.accent else Theme.surfaceBg))
        Button.focusable false
        Button.onClick (fun _ -> dispatch (TileSizeChanged ts))
    ]

let view (state: State) (dispatch: Msg -> unit) =
    let sizePx = tileSizePx state.TileSize
    DockPanel.create [
        DockPanel.children [
            StackPanel.create [
                StackPanel.dock Dock.Top
                StackPanel.orientation Orientation.Horizontal
                StackPanel.margin (Avalonia.Thickness(8.0, 4.0))
                StackPanel.children [
                    tileSizeButton "S" Small  state.TileSize dispatch
                    tileSizeButton "M" Medium state.TileSize dispatch
                    tileSizeButton "L" Large  state.TileSize dispatch
                ]
            ]
            ScrollViewer.create [
                ScrollViewer.focusable true
                ScrollViewer.onKeyDown(
                    (fun e ->
                        match e.Key with
                        | Avalonia.Input.Key.Left
                        | Avalonia.Input.Key.Right
                        | Avalonia.Input.Key.Up
                        | Avalonia.Input.Key.Down ->
                            let dir =
                                match e.Key with
                                | Avalonia.Input.Key.Left  -> Left
                                | Avalonia.Input.Key.Right -> Right
                                | Avalonia.Input.Key.Up    -> Up
                                | _                        -> Down
                            e.Handled <- true
                            let rowSize =
                                match e.Source with
                                | :? ScrollViewer as sv ->
                                    max 1 (int sv.Bounds.Width / (tileSizePx state.TileSize + 8))
                                | _ -> 1
                            dispatch (NavigateTo (dir, rowSize))
                        | Avalonia.Input.Key.Delete ->
                            state.SelectedIds
                            |> Set.iter (fun id ->
                                state.Tiles
                                |> List.tryFind (fun t -> t.File.Id = id)
                                |> Option.iter (fun tile ->
                                    if tile.File.IsOrphaned then
                                        e.Handled <- true
                                        dispatch (RemoveOrphanedFileRequested id)))
                        | Avalonia.Input.Key.E ->
                            state.AnchorId
                            |> Option.iter (fun id ->
                                e.Handled <- true
                                dispatch (EnterLoupe id))
                        | _ -> ()),
                    SubPatchOptions.OnChangeOf (state.TileSize, state.AnchorId))
                ScrollViewer.content (
                    WrapPanel.create [
                        WrapPanel.margin (Avalonia.Thickness(4.0))
                        WrapPanel.children [
                            for t in state.Tiles do
                                yield tile t sizePx (state.SelectedIds.Contains t.File.Id) state.Albums state.CurrentAlbumId dispatch
                                      :> Avalonia.FuncUI.Types.IView
                        ]
                    ])
            ]
        ]
    ]
