module IsomFolio.UI.DetailPanel

open System
open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open IsomFolio.Models

type State = {
    File        : AssetFile option
    Tags        : string list
    TagInput    : string
    IsVisible   : bool
}

type Msg =
    | FileSelected       of AssetFile
    | TagsLoaded         of string list
    | TagInputChanged    of string
    | AddTagRequested
    | RemoveTagRequested of string
    | OpenExternally
    | RevealInExplorer
    | Closed

let init () = { File = None; Tags = []; TagInput = ""; IsVisible = false }

let update (msg: Msg) (state: State) =
    match msg with
    | FileSelected f       -> 
        if state.File |> Option.map (fun existing -> existing.Id = f.Id) = Some true then
            { state with File = Some f; IsVisible = true }
        else
            { state with File = Some f; Tags = []; TagInput = ""; IsVisible = true }
    | TagsLoaded ts        -> { state with Tags = ts }
    | TagInputChanged t    -> { state with TagInput = t }
    | Closed               -> { state with IsVisible = false }
    | AddTagRequested
    | RemoveTagRequested _
    | OpenExternally
    | RevealInExplorer     -> state   // handled by MainView

let private formatBytes (bytes: int64) =
    if bytes < 1024L then $"{bytes} B"
    elif bytes < 1024L * 1024L then $"{bytes / 1024L} KB"
    else $"{bytes / 1024L / 1024L} MB"

let private formatUnix (unix: int64) =
    DateTimeOffset.FromUnixTimeSeconds(unix).LocalDateTime.ToString("yyyy-MM-dd HH:mm")

let private metaRow (label: string) (value: string) =
    DockPanel.create [
        DockPanel.margin (Avalonia.Thickness(0.0, 3.0))
        DockPanel.children [
            TextBlock.create [
                TextBlock.dock Dock.Left
                TextBlock.text label
                TextBlock.foreground (SolidColorBrush(Color.Parse("#888888")))
                TextBlock.fontSize 12.0
                TextBlock.width 72.0
            ]
            TextBlock.create [
                TextBlock.text value
                TextBlock.foreground Brushes.White
                TextBlock.fontSize 12.0
                TextBlock.textWrapping TextWrapping.Wrap
            ]
        ]
    ] :> Avalonia.FuncUI.Types.IView

let private tagChip (tag: string) (dispatch: Msg -> unit) =
    Border.create [
        Border.cornerRadius 4.0
        Border.background (SolidColorBrush(Color.Parse("#333333")))
        Border.margin (Avalonia.Thickness(0.0, 2.0, 4.0, 2.0))
        Border.child (
            StackPanel.create [
                StackPanel.orientation Orientation.Horizontal
                StackPanel.margin (Avalonia.Thickness(6.0, 3.0))
                StackPanel.children [
                    TextBlock.create [
                        TextBlock.text tag
                        TextBlock.foreground Brushes.White
                        TextBlock.fontSize 12.0
                        TextBlock.verticalAlignment VerticalAlignment.Center
                    ]
                    Button.create [
                        Button.content "×"
                        Button.fontSize 12.0
                        Button.padding (Avalonia.Thickness(4.0, 0.0))
                        Button.background Brushes.Transparent
                        Button.foreground (SolidColorBrush(Color.Parse("#AAAAAA")))
                        Button.onClick (fun _ -> dispatch (RemoveTagRequested tag))
                    ]
                ]
            ])
    ] :> Avalonia.FuncUI.Types.IView

let view (state: State) (dispatch: Msg -> unit) =
    if not state.IsVisible then Border.create [] :> Avalonia.FuncUI.Types.IView else
    DockPanel.create [
        DockPanel.width 280.0
        DockPanel.background (SolidColorBrush(Color.Parse("#1E1E1E")))
        DockPanel.children [
            DockPanel.create [
                DockPanel.dock Dock.Top
                DockPanel.margin (Avalonia.Thickness(8.0, 8.0, 8.0, 0.0))
                DockPanel.children [
                    Button.create [
                        Button.dock Dock.Right
                        Button.content "×"
                        Button.background Brushes.Transparent
                        Button.foreground (SolidColorBrush(Color.Parse("#AAAAAA")))
                        Button.onClick (fun _ -> dispatch Closed)
                    ]
                    TextBlock.create [
                        TextBlock.text "Details"
                        TextBlock.foreground Brushes.White
                        TextBlock.fontSize 14.0
                        TextBlock.fontWeight FontWeight.SemiBold
                        TextBlock.verticalAlignment VerticalAlignment.Center
                    ]
                ]
            ]
            match state.File with
            | None ->
                TextBlock.create [
                    TextBlock.text "No file selected"
                    TextBlock.foreground (SolidColorBrush(Color.Parse("#888888")))
                    TextBlock.horizontalAlignment HorizontalAlignment.Center
                    TextBlock.verticalAlignment VerticalAlignment.Center
                ]
            | Some f ->
                ScrollViewer.create [
                    ScrollViewer.content (
                        StackPanel.create [
                            StackPanel.margin (Avalonia.Thickness(12.0, 8.0))
                            StackPanel.children [
                                yield metaRow "Name"     f.Name
                                yield metaRow "Size"     (formatBytes f.SizeBytes)
                                yield metaRow "Modified" (formatUnix f.MTimeUnix)
                                yield metaRow "Path"     f.Folder
                                yield TextBlock.create [
                                    TextBlock.text "TAGS"
                                    TextBlock.foreground (SolidColorBrush(Color.Parse("#888888")))
                                    TextBlock.fontSize 11.0
                                    TextBlock.margin (Avalonia.Thickness(0.0, 12.0, 0.0, 4.0))
                                ] :> Avalonia.FuncUI.Types.IView
                                yield WrapPanel.create [
                                    WrapPanel.children [
                                        for tag in state.Tags do
                                            yield tagChip tag dispatch
                                    ]
                                ] :> Avalonia.FuncUI.Types.IView
                                yield DockPanel.create [
                                    DockPanel.margin (Avalonia.Thickness(0.0, 4.0))
                                    DockPanel.children [
                                        Button.create [
                                            Button.dock Dock.Right
                                            Button.content "Add"
                                            Button.isEnabled (state.TagInput.Trim() <> "")
                                            Button.onClick (fun _ -> dispatch AddTagRequested)
                                        ]
                                        TextBox.create [
                                            TextBox.text state.TagInput
                                            TextBox.watermark "Add tag…"
                                            TextBox.onTextChanged (fun t -> dispatch (TagInputChanged t))
                                            TextBox.onKeyDown (fun e ->
                                                if e.Key = Avalonia.Input.Key.Enter then
                                                    dispatch AddTagRequested)
                                        ]
                                    ]
                                ] :> Avalonia.FuncUI.Types.IView
                                yield StackPanel.create [
                                    StackPanel.orientation Orientation.Horizontal
                                    StackPanel.margin (Avalonia.Thickness(0.0, 12.0))
                                    StackPanel.children [
                                        Button.create [
                                            Button.content "Open"
                                            Button.margin (Avalonia.Thickness(0.0, 0.0, 4.0, 0.0))
                                            Button.onClick (fun _ -> dispatch OpenExternally)
                                        ]
                                        Button.create [
                                            Button.content "Reveal"
                                            Button.onClick (fun _ -> dispatch RevealInExplorer)
                                        ]
                                    ]
                                ] :> Avalonia.FuncUI.Types.IView
                            ]
                        ])
                ]
        ]
    ] :> Avalonia.FuncUI.Types.IView
