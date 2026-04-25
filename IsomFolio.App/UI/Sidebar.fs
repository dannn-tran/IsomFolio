module IsomFolio.UI.Sidebar

open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media

type State = {
    Folders      : string list
    Tags         : (string * int) list   // (name, count)
    SelectedTags : string list
}

type Msg =
    | OpenFolderRequested
    | FolderRemoved      of string
    | TagToggled         of string
    | FoldersLoaded      of string list
    | TagsLoaded         of (string * int) list

let init () = { Folders = []; Tags = []; SelectedTags = [] }

let update (msg: Msg) (state: State) =
    match msg with
    | FoldersLoaded fs  -> { state with Folders = fs }
    | TagsLoaded ts     -> { state with Tags = ts }
    | FolderRemoved f   -> { state with Folders = state.Folders |> List.filter ((<>) f) }
    | TagToggled tag    ->
        let selected =
            if state.SelectedTags |> List.contains tag
            then state.SelectedTags |> List.filter ((<>) tag)
            else state.SelectedTags @ [ tag ]
        { state with SelectedTags = selected }
    | OpenFolderRequested -> state

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

let view (state: State) (dispatch: Msg -> unit) =
    DockPanel.create [
        DockPanel.width 220.0
        DockPanel.background (SolidColorBrush(Color.Parse("#1E1E1E")))
        DockPanel.children [
            Button.create [
                Button.dock Dock.Top
                Button.content "Open Folder…"
                Button.horizontalAlignment HorizontalAlignment.Stretch
                Button.margin (Avalonia.Thickness(8.0))
                Button.onClick (fun _ -> dispatch OpenFolderRequested)
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
                            for folder in state.Folders do
                                yield TextBlock.create [
                                    TextBlock.text (System.IO.Path.GetFileName(folder))
                                    TextBlock.foreground Brushes.White
                                    TextBlock.fontSize 13.0
                                    TextBlock.margin (Avalonia.Thickness(0.0, 2.0))
                                    TextBlock.tip folder
                                ] :> Avalonia.FuncUI.Types.IView
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
