module IsomFolio.UI.TagBrowser

open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media

type State = {
    AllTags      : (string * int) list
    Filter       : string
    RenameInput  : (string * string) option   // (originalTag, currentText)
    PendingDelete : string option
}

type Msg =
    | TagsLoaded        of (string * int) list
    | FilterChanged     of string
    | RenameStarted     of string
    | RenameTextChanged of string
    | RenameSubmitted
    | RenameCancelled
    | DeleteArmed       of string
    | DeleteConfirmed
    | DeleteCancelled
    | MutationCompleted of (string * int) list
    | Closed

let init () = {
    AllTags       = []
    Filter        = ""
    RenameInput   = None
    PendingDelete = None
}

let update (msg: Msg) (state: State) : State =
    match msg with
    | TagsLoaded tags      -> { state with AllTags = tags }
    | FilterChanged f      -> { state with Filter = f }
    | RenameStarted tag    -> { state with RenameInput = Some (tag, tag); PendingDelete = None }
    | RenameTextChanged t  ->
        match state.RenameInput with
        | None -> state
        | Some (orig, _) -> { state with RenameInput = Some (orig, t) }
    | RenameSubmitted      -> state  // async — handled in MainView
    | RenameCancelled      -> { state with RenameInput = None }
    | DeleteArmed tag      -> { state with PendingDelete = Some tag; RenameInput = None }
    | DeleteConfirmed      -> state  // async — handled in MainView
    | DeleteCancelled      -> { state with PendingDelete = None }
    | MutationCompleted tags ->
        { state with AllTags = tags; RenameInput = None; PendingDelete = None }
    | Closed               -> state  // handled in MainView

let filteredTags (state: State) : (string * int) list =
    let f = state.Filter.Trim().ToLowerInvariant()
    if f = "" then state.AllTags
    else state.AllTags |> List.filter (fun (tag, _) -> tag.ToLowerInvariant().Contains(f))

let private hasDescendants (tag: string) (allTags: (string * int) list) =
    allTags |> List.exists (fun (t, _) -> t.StartsWith(tag + "/"))

let private subtreeSize (tag: string) (allTags: (string * int) list) =
    allTags |> List.filter (fun (t, _) -> t = tag || t.StartsWith(tag + "/")) |> List.length

let private iconBtn (label: string) (onClick: unit -> unit) =
    Button.create [
        Button.content label
        Button.background Brushes.Transparent
        Button.foreground (SolidColorBrush(Theme.textMuted))
        Button.fontSize Theme.FontSize.xs
        Button.padding (Avalonia.Thickness(4.0, 1.0))
        Button.minWidth 0.0
        Button.onClick (fun _ -> onClick ())
    ]

let private tagRow
    (tag: string)
    (count: int)
    (state: State)
    (dispatch: Msg -> unit)
    : Avalonia.FuncUI.Types.IView =

    let isRenaming = state.RenameInput |> Option.map fst = Some tag
    let isPendingDelete = state.PendingDelete = Some tag

    if isRenaming then
        let currentText = state.RenameInput |> Option.map snd |> Option.defaultValue tag
        DockPanel.create [
            DockPanel.margin (Avalonia.Thickness(0.0, 2.0))
            DockPanel.children [
                StackPanel.create [
                    StackPanel.dock Dock.Right
                    StackPanel.orientation Orientation.Horizontal
                    StackPanel.children [
                        iconBtn "✓" (fun () -> dispatch RenameSubmitted)
                        iconBtn "✕" (fun () -> dispatch RenameCancelled)
                    ]
                ]
                TextBox.create [
                    TextBox.text currentText
                    TextBox.fontSize Theme.FontSize.sm
                    TextBox.onTextChanged (fun t -> dispatch (RenameTextChanged t))
                    TextBox.onKeyDown (fun e ->
                        match e.Key with
                        | Avalonia.Input.Key.Return -> dispatch RenameSubmitted
                        | Avalonia.Input.Key.Escape -> dispatch RenameCancelled
                        | _ -> ())
                ]
            ]
        ] :> Avalonia.FuncUI.Types.IView

    elif isPendingDelete then
        let n = subtreeSize tag state.AllTags
        let msg =
            if hasDescendants tag state.AllTags then $"Delete \"{tag}\" and subtags ({n})?"
            else $"Delete \"{tag}\" from {count} files?"
        DockPanel.create [
            DockPanel.margin (Avalonia.Thickness(0.0, 2.0))
            DockPanel.children [
                StackPanel.create [
                    StackPanel.dock Dock.Right
                    StackPanel.orientation Orientation.Horizontal
                    StackPanel.children [
                        iconBtn "✓" (fun () -> dispatch DeleteConfirmed)
                        iconBtn "✕" (fun () -> dispatch DeleteCancelled)
                    ]
                ]
                TextBlock.create [
                    TextBlock.text msg
                    TextBlock.foreground Brushes.White
                    TextBlock.fontSize Theme.FontSize.sm
                    TextBlock.verticalAlignment VerticalAlignment.Center
                ]
            ]
        ] :> Avalonia.FuncUI.Types.IView

    else
        DockPanel.create [
            DockPanel.margin (Avalonia.Thickness(0.0, 2.0))
            DockPanel.children [
                StackPanel.create [
                    StackPanel.dock Dock.Right
                    StackPanel.orientation Orientation.Horizontal
                    StackPanel.children [
                        iconBtn "✎" (fun () -> dispatch (RenameStarted tag))
                        iconBtn "✗" (fun () -> dispatch (DeleteArmed tag))
                    ]
                ]
                TextBlock.create [
                    TextBlock.dock Dock.Left
                    TextBlock.text tag
                    TextBlock.foreground (SolidColorBrush(Theme.textDim))
                    TextBlock.fontSize Theme.FontSize.sm
                    TextBlock.verticalAlignment VerticalAlignment.Center
                ]
                TextBlock.create [
                    TextBlock.text (string count)
                    TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                    TextBlock.fontSize Theme.FontSize.xs
                    TextBlock.verticalAlignment VerticalAlignment.Center
                    TextBlock.horizontalAlignment HorizontalAlignment.Right
                    TextBlock.margin (Avalonia.Thickness(0.0, 0.0, 8.0, 0.0))
                ]
            ]
        ] :> Avalonia.FuncUI.Types.IView

let view (state: State) (dispatch: Msg -> unit) : Avalonia.FuncUI.Types.IView =
    Border.create [
        Border.horizontalAlignment HorizontalAlignment.Center
        Border.verticalAlignment VerticalAlignment.Center
        Border.width 440.0
        Border.maxHeight 560.0
        Border.background (SolidColorBrush(Theme.panelBg))
        Border.cornerRadius 8.0
        Border.child (
            DockPanel.create [
                DockPanel.children [
                    // Header
                    DockPanel.create [
                        DockPanel.dock Dock.Top
                        DockPanel.margin (Avalonia.Thickness(16.0, 12.0, 12.0, 12.0))
                        DockPanel.children [
                            Button.create [
                                Button.dock Dock.Right
                                Button.content "×"
                                Button.background Brushes.Transparent
                                Button.foreground (SolidColorBrush(Theme.textDim))
                                Button.padding (Avalonia.Thickness(6.0, 0.0))
                                Button.onClick (fun _ -> dispatch Closed)
                            ]
                            TextBlock.create [
                                TextBlock.text "Tag Browser"
                                TextBlock.foreground Brushes.White
                                TextBlock.fontSize Theme.FontSize.lg
                                TextBlock.fontWeight FontWeight.SemiBold
                                TextBlock.verticalAlignment VerticalAlignment.Center
                            ]
                        ]
                    ]
                    // Filter box
                    TextBox.create [
                        TextBox.dock Dock.Top
                        TextBox.text state.Filter
                        TextBox.watermark "Filter tags…"
                        TextBox.fontSize Theme.FontSize.sm
                        TextBox.margin (Avalonia.Thickness(12.0, 0.0, 12.0, 8.0))
                        TextBox.onTextChanged (fun t -> dispatch (FilterChanged t))
                    ]
                    // Tag list
                    ScrollViewer.create [
                        ScrollViewer.content (
                            StackPanel.create [
                                StackPanel.margin (Avalonia.Thickness(12.0, 0.0, 12.0, 12.0))
                                StackPanel.children [
                                    let tags = filteredTags state
                                    if tags.IsEmpty then
                                        yield TextBlock.create [
                                            TextBlock.text (if state.AllTags.IsEmpty then "No tags in catalog." else "No tags match filter.")
                                            TextBlock.foreground (SolidColorBrush(Theme.textMuted))
                                            TextBlock.fontSize Theme.FontSize.sm
                                            TextBlock.margin (Avalonia.Thickness(0.0, 8.0))
                                        ] :> Avalonia.FuncUI.Types.IView
                                    else
                                        for (tag, count) in tags do
                                            yield tagRow tag count state dispatch
                                ]
                            ])
                    ]
                ]
            ])
    ] :> Avalonia.FuncUI.Types.IView
