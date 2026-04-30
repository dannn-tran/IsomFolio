module IsomFolio.UI.GridView

open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open Avalonia.Media.Imaging
open IsomFolio.Models
open IsomFolio.FileIndex

type TileModel = {
    File      : AssetFile
    Thumbnail : ThumbnailState
}

type State = {
    Tiles      : TileModel list
    TileSize   : TileSize
    SelectedId : FileId option
}

type NavDirection = Left | Right | Up | Down

type Msg =
    | TilesLoaded        of AssetFile list
    | ThumbnailUpdated   of FileId * ThumbnailState
    | TileSizeChanged    of TileSize
    | TileSelected       of FileId
    | NavigateTo         of NavDirection * int

let init () = { Tiles = []; TileSize = Medium; SelectedId = None }

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
        let newSelectedId =
            state.SelectedId
            |> Option.filter (fun id -> files |> List.exists (fun f -> f.Id = id))
        { state with Tiles = tiles; SelectedId = newSelectedId }
    | ThumbnailUpdated(fileId, thumbState) ->
        let tiles =
            state.Tiles |> List.map (fun t ->
                if t.File.Id = fileId then { t with Thumbnail = thumbState } else t)
        { state with Tiles = tiles }
    | TileSizeChanged ts  -> { state with TileSize = ts }
    | TileSelected id     -> { state with SelectedId = Some id }
    | NavigateTo (dir, rowSize) ->
        if state.Tiles.IsEmpty then state
        else
            let currentIdx =
                state.SelectedId
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
            { state with SelectedId = Some state.Tiles.[newIdx].File.Id }

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

let private tile (model: TileModel) (sizePx: int) (selected: bool) (dispatch: Msg -> unit) =
    let px = float sizePx
    Border.create [
        Border.width px
        Border.height (px + 24.0)
        Border.margin (Avalonia.Thickness(4.0))
        Border.cornerRadius 4.0
        Border.background (if selected then SolidColorBrush(Color.Parse("#0078D4")) else SolidColorBrush(Color.Parse("#2D2D2D")))
        Border.onTapped(
            (fun e ->
                dispatch (TileSelected model.File.Id)
                match e.Source with
                | :? Avalonia.Visual as v ->
                    let sv = Avalonia.VisualTree.VisualExtensions.FindAncestorOfType<ScrollViewer>(v, true)
                    if not (isNull sv) then sv.Focus() |> ignore
                | _ -> ()),
            SubPatchOptions.OnChangeOf model.File.Id)
        Border.child (
            DockPanel.create [
                DockPanel.children [
                    TextBlock.create [
                        TextBlock.dock Dock.Bottom
                        TextBlock.text model.File.Name
                        TextBlock.fontSize 11.0
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
                            TextBlock.foreground (SolidColorBrush(Color.Parse("#888888")))
                            TextBlock.horizontalAlignment HorizontalAlignment.Center
                            TextBlock.verticalAlignment VerticalAlignment.Center
                        ]
                    | NotRequested ->
                        Border.create [
                            Border.background (SolidColorBrush(Color.Parse("#3D3D3D")))
                            Border.horizontalAlignment HorizontalAlignment.Stretch
                            Border.verticalAlignment VerticalAlignment.Stretch
                        ]
                ]
            ])
    ]

let private tileSizeButton (label: string) (ts: TileSize) (current: TileSize) (dispatch: Msg -> unit) =
    Button.create [
        Button.content label
        Button.margin (Avalonia.Thickness(2.0, 0.0))
        Button.background (if ts = current then "#0078D4" else "#3D3D3D")
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
                        let dir =
                            match e.Key with
                            | Avalonia.Input.Key.Left  -> Some Left
                            | Avalonia.Input.Key.Right -> Some Right
                            | Avalonia.Input.Key.Up    -> Some Up
                            | Avalonia.Input.Key.Down  -> Some Down
                            | _ -> None
                        match dir with
                        | Some d ->
                            e.Handled <- true
                            let rowSize =
                                match e.Source with
                                | :? ScrollViewer as sv ->
                                    max 1 (int sv.Bounds.Width / (tileSizePx state.TileSize + 8))
                                | _ -> 1
                            dispatch (NavigateTo (d, rowSize))
                        | None -> ()),
                    SubPatchOptions.OnChangeOf state.TileSize)
                ScrollViewer.content (
                    WrapPanel.create [
                        WrapPanel.margin (Avalonia.Thickness(4.0))
                        WrapPanel.children [
                            for t in state.Tiles do
                                yield tile t sizePx (state.SelectedId = Some t.File.Id) dispatch
                                      :> Avalonia.FuncUI.Types.IView
                        ]
                    ])
            ]
        ]
    ]
