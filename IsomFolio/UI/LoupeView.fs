module IsomFolio.UI.LoupeView

open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open Avalonia.Media.Imaging
open IsomFolio.Core.Models

type Msg =
    | Navigate of GridView.NavDirection
    | JumpTo   of int
    | ExitRequested

let private bitmapCache = System.Collections.Generic.Dictionary<string, Bitmap>()

let clearCache () =
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

let private filmstripThumb (idx: int) (model: GridView.TileModel) (selected: bool) (dispatch: Msg -> unit) =
    let px = 72.0
    Border.create [
        Border.width px
        Border.height px
        Border.margin (Avalonia.Thickness(3.0, 4.0))
        Border.cornerRadius 3.0
        Border.borderBrush (SolidColorBrush(if selected then Theme.accent else Avalonia.Media.Colors.Transparent))
        Border.borderThickness (Avalonia.Thickness(if selected then 2.0 else 0.0))
        Border.background (SolidColorBrush(Theme.tileBg))
        Border.cursor Avalonia.Input.Cursor.Default
        Border.onTapped(
            (fun _ -> dispatch (JumpTo idx)),
            SubPatchOptions.OnChangeOf idx)
        Border.child (
            match model.Thumbnail with
            | Ready path ->
                match tryLoadBitmap path with
                | Some bmp ->
                    Image.create [
                        Image.source bmp
                        Image.stretch Stretch.UniformToFill
                        Image.horizontalAlignment HorizontalAlignment.Center
                        Image.verticalAlignment VerticalAlignment.Center
                    ] :> Avalonia.FuncUI.Types.IView
                | None ->
                    TextBlock.create [
                        TextBlock.text "⚠"
                        TextBlock.horizontalAlignment HorizontalAlignment.Center
                        TextBlock.verticalAlignment VerticalAlignment.Center
                    ] :> Avalonia.FuncUI.Types.IView
            | _ ->
                Border.create [
                    Border.background (SolidColorBrush(Theme.surfaceBg))
                    Border.horizontalAlignment HorizontalAlignment.Stretch
                    Border.verticalAlignment VerticalAlignment.Stretch
                ] :> Avalonia.FuncUI.Types.IView)
    ]

let view (tiles: GridView.TileModel list) (selectedIdx: int) (dispatch: Msg -> unit) =
    let selected = if tiles.IsEmpty then None else Some tiles.[selectedIdx]
    DockPanel.create [
        DockPanel.background (SolidColorBrush(Theme.mainBg))
        DockPanel.children [
            // Top toolbar
            Border.create [
                Border.dock Dock.Top
                Border.background (SolidColorBrush(Theme.panelBg))
                Border.height 40.0
                Border.child (
                    DockPanel.create [
                        DockPanel.margin (Avalonia.Thickness(8.0, 0.0))
                        DockPanel.children [
                            Button.create [
                                Button.dock Dock.Right
                                Button.content "✕"
                                Button.fontSize Theme.FontSize.lg
                                Button.foreground (SolidColorBrush(Theme.textMuted))
                                Button.background Brushes.Transparent
                                Button.borderThickness (Avalonia.Thickness(0.0))
                                Button.padding (Avalonia.Thickness(8.0, 4.0))
                                Button.tip "Exit loupe (Esc)"
                                Button.onClick (fun _ -> dispatch ExitRequested)
                            ]
                            StackPanel.create [
                                StackPanel.dock Dock.Right
                                StackPanel.orientation Orientation.Horizontal
                                StackPanel.children [
                                    Button.create [
                                        Button.content "←"
                                        Button.fontSize Theme.FontSize.lg
                                        Button.foreground (SolidColorBrush(Theme.textMuted))
                                        Button.background Brushes.Transparent
                                        Button.borderThickness (Avalonia.Thickness(0.0))
                                        Button.padding (Avalonia.Thickness(8.0, 4.0))
                                        Button.isEnabled (selectedIdx > 0)
                                        Button.onClick(
                                            (fun _ -> dispatch (Navigate GridView.Left)),
                                            SubPatchOptions.OnChangeOf selectedIdx)
                                    ]
                                    Button.create [
                                        Button.content "→"
                                        Button.fontSize Theme.FontSize.lg
                                        Button.foreground (SolidColorBrush(Theme.textMuted))
                                        Button.background Brushes.Transparent
                                        Button.borderThickness (Avalonia.Thickness(0.0))
                                        Button.padding (Avalonia.Thickness(8.0, 4.0))
                                        Button.isEnabled (selectedIdx < tiles.Length - 1)
                                        Button.onClick(
                                            (fun _ -> dispatch (Navigate GridView.Right)),
                                            SubPatchOptions.OnChangeOf selectedIdx)
                                    ]
                                ]
                            ]
                            TextBlock.create [
                                TextBlock.text (selected |> Option.map (fun m -> m.File.Name) |> Option.defaultValue "")
                                TextBlock.foreground Brushes.White
                                TextBlock.fontSize Theme.FontSize.md
                                TextBlock.verticalAlignment VerticalAlignment.Center
                                TextBlock.textTrimming TextTrimming.CharacterEllipsis
                            ]
                        ]
                    ])
            ] :> Avalonia.FuncUI.Types.IView
            // Bottom filmstrip
            Border.create [
                Border.dock Dock.Bottom
                Border.background (SolidColorBrush(Theme.panelBg))
                Border.height 88.0
                Border.child (
                    ScrollViewer.create [
                        ScrollViewer.horizontalScrollBarVisibility Avalonia.Controls.Primitives.ScrollBarVisibility.Auto
                        ScrollViewer.verticalScrollBarVisibility Avalonia.Controls.Primitives.ScrollBarVisibility.Disabled
                        ScrollViewer.content (
                            StackPanel.create [
                                StackPanel.orientation Orientation.Horizontal
                                StackPanel.margin (Avalonia.Thickness(4.0, 0.0))
                                StackPanel.children [
                                    for i, model in tiles |> List.indexed do
                                        yield filmstripThumb i model (i = selectedIdx) dispatch
                                              :> Avalonia.FuncUI.Types.IView
                                ]
                            ])
                    ])
            ] :> Avalonia.FuncUI.Types.IView
            // Center: large image
            Border.create [
                Border.background (SolidColorBrush(Color.FromRgb(10uy, 10uy, 10uy)))
                Border.child (
                    match selected with
                    | None ->
                        TextBlock.create [
                            TextBlock.text "No image selected"
                            TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                            TextBlock.horizontalAlignment HorizontalAlignment.Center
                            TextBlock.verticalAlignment VerticalAlignment.Center
                        ] :> Avalonia.FuncUI.Types.IView
                    | Some model ->
                        match tryLoadBitmap model.File.Path with
                        | Some bmp ->
                            Image.create [
                                Image.source bmp
                                Image.stretch Stretch.Uniform
                                Image.horizontalAlignment HorizontalAlignment.Center
                                Image.verticalAlignment VerticalAlignment.Center
                            ] :> Avalonia.FuncUI.Types.IView
                        | None ->
                            TextBlock.create [
                                TextBlock.text "⚠ Could not load image"
                                TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                TextBlock.fontSize Theme.FontSize.lg
                                TextBlock.horizontalAlignment HorizontalAlignment.Center
                                TextBlock.verticalAlignment VerticalAlignment.Center
                            ] :> Avalonia.FuncUI.Types.IView)
            ]
        ]
    ]
