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

type Msg =
    | TilesLoaded        of AssetFile list
    | ThumbnailUpdated   of FileId * ThumbnailState
    | TileSizeChanged    of TileSize
    | TileSelected       of FileId

let init () = { Tiles = []; TileSize = Medium; SelectedId = None }

let update (msg: Msg) (state: State) =
    match msg with
    | TilesLoaded files ->
        let tiles = files |> List.map (fun f -> { File = f; Thumbnail = NotRequested })
        { state with Tiles = tiles; SelectedId = None }
    | ThumbnailUpdated(fileId, thumbState) ->
        let tiles =
            state.Tiles |> List.map (fun t ->
                if t.File.Id = fileId then { t with Thumbnail = thumbState } else t)
        { state with Tiles = tiles }
    | TileSizeChanged ts -> { state with TileSize = ts }
    | TileSelected id    -> { state with SelectedId = Some id }

let private tryLoadBitmap (path: string) : Bitmap option =
    try Some(new Bitmap(path))
    with _ -> None

let private tile (model: TileModel) (sizePx: int) (selected: bool) (dispatch: Msg -> unit) =
    let px = float sizePx
    Border.create [
        Border.width px
        Border.height (px + 24.0)
        Border.margin (Avalonia.Thickness(4.0))
        Border.cornerRadius 4.0
        Border.background (if selected then SolidColorBrush(Color.Parse("#0078D4")) else SolidColorBrush(Color.Parse("#2D2D2D")))
        Border.onPointerPressed (fun _ -> dispatch (TileSelected model.File.Id))
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
