module IsomFolio.UI.TagTree

open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout
open Avalonia.Media
open IsomFolio.Core.Metadata.TagTree

type State = {
    Roots                : TagNode list
    Expanded             : Set<string>
    AddInput             : (string * string) option   // (parentPath, text); "" parentPath = root
    PendingRemoveSubtree : string option              // fullPath awaiting 🗑 confirmation
    AllTags              : string list                // catalog-wide tags for autocomplete
}

type Msg =
    | TagAdded               of string
    | TagRemoved             of string
    | SubtreeRemoveArmed     of string
    | SubtreeRemoveConfirmed
    | SubtreeRemoveCancelled
    | TagRetagged            of string
    | NodeToggled            of string
    | AddInputOpened         of string
    | AddInputChanged        of string
    | AddInputSubmitted
    | AddInputCancelled
    | AllTagsLoaded          of string list
    | SuggestionSelected     of string

let init () = {
    Roots                = []
    Expanded             = Set.empty
    AddInput             = None
    PendingRemoveSubtree = None
    AllTags              = []
}

let fromTagList (tags: string list) : State =
    let roots = buildTree tags
    let allPaths =
        let rec collect (nodes: TagNode list) =
            nodes |> List.collect (fun n -> n.FullPath :: collect n.Children)
        collect roots
    { init () with
        Roots    = roots
        Expanded = Set.ofList allPaths }

// True for Msg values that mutate the tag set and must be persisted.
let isMutating (msg: Msg) =
    match msg with
    | TagAdded _ | TagRemoved _ | SubtreeRemoveConfirmed | TagRetagged _ -> true
    | _ -> false

let update (msg: Msg) (state: State) : State =
    match msg with
    | TagAdded path ->
        { state with
            Roots    = addTag path state.Roots
            Expanded = state.Expanded |> Set.add path
            AddInput = None }
    | TagRemoved path ->
        { state with Roots = removeTag path state.Roots }
    | SubtreeRemoveArmed path ->
        { state with PendingRemoveSubtree = Some path }
    | SubtreeRemoveConfirmed ->
        match state.PendingRemoveSubtree with
        | None -> state
        | Some path ->
            { state with
                Roots                = removeSubtree path state.Roots
                PendingRemoveSubtree = None }
    | SubtreeRemoveCancelled ->
        { state with PendingRemoveSubtree = None }
    | TagRetagged path ->
        { state with Roots = reTag path state.Roots }
    | NodeToggled path ->
        let expanded =
            if state.Expanded |> Set.contains path
            then state.Expanded |> Set.remove path
            else state.Expanded |> Set.add path
        { state with Expanded = expanded }
    | AddInputOpened parentPath ->
        { state with AddInput = Some (parentPath, ""); PendingRemoveSubtree = None }
    | AddInputChanged text ->
        match state.AddInput with
        | None -> state
        | Some (parent, _) -> { state with AddInput = Some (parent, text) }
    | AddInputSubmitted ->
        match state.AddInput with
        | None -> state
        | Some (parent, text) ->
            let trimmed = text.Trim()
            if trimmed = "" then { state with AddInput = None }
            else
                let fullPath = if parent = "" then trimmed else parent + "/" + trimmed
                { state with
                    Roots    = addTag fullPath state.Roots
                    Expanded = state.Expanded |> Set.add fullPath
                    AddInput = None }
    | AddInputCancelled ->
        { state with AddInput = None }
    | AllTagsLoaded tags ->
        { state with AllTags = tags }
    | SuggestionSelected fullPath ->
        match state.AddInput with
        | None -> state
        | Some (parent, _) ->
            let text =
                if parent = "" then fullPath
                elif fullPath.StartsWith(parent + "/") then fullPath.[(parent.Length + 1)..]
                else fullPath
            { state with AddInput = Some (parent, text) }

// Returns autocomplete suggestions for the current AddInput state.
// Filters AllTags by prefix match, excludes already-tagged paths, max 8.
let suggestions (state: State) : string list =
    match state.AddInput with
    | None -> []
    | Some (parent, text) ->
        let trimmed = text.Trim().ToLowerInvariant()
        if trimmed = "" then []
        else
            let prefix = if parent = "" then trimmed else (parent + "/" + trimmed).ToLowerInvariant()
            let existing = flattenTree state.Roots |> Set.ofList
            state.AllTags
            |> List.filter (fun t ->
                t.ToLowerInvariant().StartsWith(prefix) && not (existing |> Set.contains t))
            |> List.truncate 8

// Count all Tagged descendants (not counting the node itself).
let rec private taggedDescendantCount (nodes: TagNode list) =
    nodes |> List.sumBy (fun n ->
        (if n.Kind = Tagged then 1 else 0) + taggedDescendantCount n.Children)

let private iconBtn (label: string) (onClick: unit -> unit) =
    Button.create [
        Button.content label
        Button.background Brushes.Transparent
        Button.foreground (SolidColorBrush(Theme.textMuted))
        Button.fontSize Theme.FontSize.xs
        Button.padding (Avalonia.Thickness(3.0, 0.0))
        Button.minWidth 0.0
        Button.onClick (fun _ -> onClick ())
    ]

let rec private nodeView
    (node: TagNode)
    (state: State)
    (dispatch: Msg -> unit)
    (depth: int)
    : Avalonia.FuncUI.Types.IView =

    let isExpanded = state.Expanded |> Set.contains node.FullPath
    let isPending  = state.PendingRemoveSubtree = Some node.FullPath
    let hasChildren = not node.Children.IsEmpty
    let isGhost = node.Kind = Ghost
    let indent = float (depth * 16)

    let subtreeCount =
        (if node.Kind = Tagged then 1 else 0) + taggedDescendantCount node.Children

    StackPanel.create [
        StackPanel.orientation Orientation.Vertical
        StackPanel.children [
            DockPanel.create [
                DockPanel.margin (Avalonia.Thickness(indent, 1.0, 0.0, 1.0))
                DockPanel.children [
                    // Right-side action buttons
                    StackPanel.create [
                        StackPanel.dock Dock.Right
                        StackPanel.orientation Orientation.Horizontal
                        StackPanel.children [
                            // Pending 🗑 confirmation
                            if isPending then
                                Button.create [
                                    Button.content $"Remove all ({subtreeCount}) ✓"
                                    Button.background (SolidColorBrush(Theme.warningBg))
                                    Button.foreground Brushes.White
                                    Button.fontSize Theme.FontSize.xs
                                    Button.padding (Avalonia.Thickness(4.0, 1.0))
                                    Button.minWidth 0.0
                                    Button.onClick (fun _ -> dispatch SubtreeRemoveConfirmed)
                                ]
                                iconBtn "✕" (fun () -> dispatch SubtreeRemoveCancelled)
                            else
                                // ⊕ re-tag (Ghost only)
                                if isGhost then
                                    iconBtn "⊕" (fun () -> dispatch (TagRetagged node.FullPath))
                                // ✕ remove/demote (Tagged only)
                                if not isGhost then
                                    iconBtn "✕" (fun () -> dispatch (TagRemoved node.FullPath))
                                // 🗑 remove subtree (parents and Ghost nodes)
                                if hasChildren || isGhost then
                                    iconBtn "🗑" (fun () -> dispatch (SubtreeRemoveArmed node.FullPath))
                                // + add child
                                iconBtn "+" (fun () -> dispatch (AddInputOpened node.FullPath))
                        ]
                    ]
                    // Expand toggle + label
                    StackPanel.create [
                        StackPanel.orientation Orientation.Horizontal
                        StackPanel.verticalAlignment VerticalAlignment.Center
                        StackPanel.children [
                            if hasChildren then
                                Button.create [
                                    Button.content (if isExpanded then "▾" else "▸")
                                    Button.background Brushes.Transparent
                                    Button.foreground (SolidColorBrush(Theme.textMuted))
                                    Button.fontSize Theme.FontSize.xs
                                    Button.padding (Avalonia.Thickness(0.0, 0.0, 4.0, 0.0))
                                    Button.minWidth 0.0
                                    Button.onClick (fun _ -> dispatch (NodeToggled node.FullPath))
                                ]
                            else
                                Border.create [ Border.width 16.0 ]
                            TextBlock.create [
                                TextBlock.text (
                                    if not isExpanded && hasChildren then
                                        $"{node.Segment} ({taggedDescendantCount node.Children})"
                                    else
                                        node.Segment)
                                TextBlock.foreground (
                                    SolidColorBrush(if isGhost then Theme.textMuted else Theme.textDim))
                                TextBlock.fontSize Theme.FontSize.sm
                                TextBlock.verticalAlignment VerticalAlignment.Center
                            ]
                        ]
                    ]
                ]
            ] :> Avalonia.FuncUI.Types.IView

            // Inline add-input for this node's children
            match state.AddInput with
            | Some (parent, text) when parent = node.FullPath ->
                yield addInputView parent text (suggestions state) (depth + 1) dispatch
            | _ -> ()

            // Children
            if isExpanded && hasChildren then
                for child in node.Children do
                    yield nodeView child state dispatch (depth + 1)
        ]
    ] :> Avalonia.FuncUI.Types.IView

and private addInputView (parentPath: string) (text: string) (suggs: string list) (depth: int) (dispatch: Msg -> unit) =
    let indent = float (depth * 16)
    StackPanel.create [
        StackPanel.orientation Orientation.Vertical
        StackPanel.margin (Avalonia.Thickness(indent, 2.0, 4.0, 2.0))
        StackPanel.children [
            DockPanel.create [
                DockPanel.children [
                    iconBtn "✕" (fun () -> dispatch AddInputCancelled)
                    |> fun b -> DockPanel.create [ DockPanel.dock Dock.Right; DockPanel.children [ b ] ] :> Avalonia.FuncUI.Types.IView
                    TextBox.create [
                        TextBox.text text
                        TextBox.fontSize Theme.FontSize.sm
                        TextBox.watermark (if parentPath = "" then "tag or tag/subtag" else "subtag")
                        TextBox.onTextChanged (fun t -> dispatch (AddInputChanged t))
                        TextBox.onKeyDown (fun e ->
                            match e.Key with
                            | Avalonia.Input.Key.Return -> dispatch AddInputSubmitted
                            | Avalonia.Input.Key.Escape -> dispatch AddInputCancelled
                            | _ -> ())
                    ]
                ]
            ] :> Avalonia.FuncUI.Types.IView
            if not suggs.IsEmpty then
                Border.create [
                    Border.background (SolidColorBrush(Theme.panelBg))
                    Border.borderBrush (SolidColorBrush(Theme.textMuted))
                    Border.borderThickness (Avalonia.Thickness(1.0))
                    Border.cornerRadius 4.0
                    Border.child (
                        StackPanel.create [
                            StackPanel.orientation Orientation.Vertical
                            StackPanel.children [
                                for s in suggs do
                                    Button.create [
                                        Button.content s
                                        Button.background Brushes.Transparent
                                        Button.foreground Brushes.White
                                        Button.fontSize Theme.FontSize.sm
                                        Button.horizontalAlignment HorizontalAlignment.Stretch
                                        Button.horizontalContentAlignment HorizontalAlignment.Left
                                        Button.padding (Avalonia.Thickness(6.0, 3.0))
                                        Button.onClick (fun _ -> dispatch (SuggestionSelected s))
                                    ] :> Avalonia.FuncUI.Types.IView
                            ]
                        ])
                ] :> Avalonia.FuncUI.Types.IView
        ]
    ] :> Avalonia.FuncUI.Types.IView

let view (state: State) (dispatch: Msg -> unit) : Avalonia.FuncUI.Types.IView =
    StackPanel.create [
        StackPanel.orientation Orientation.Vertical
        StackPanel.children [
            for node in state.Roots do
                yield nodeView node state dispatch 0
            // Root-level add input
            match state.AddInput with
            | Some ("", text) ->
                yield addInputView "" text (suggestions state) 0 dispatch
            | _ ->
                if state.AddInput.IsNone then
                    yield Button.create [
                        Button.content "+ add tag"
                        Button.background Brushes.Transparent
                        Button.foreground (SolidColorBrush(Theme.textMuted))
                        Button.fontSize Theme.FontSize.xs
                        Button.padding (Avalonia.Thickness(0.0, 4.0))
                        Button.horizontalAlignment HorizontalAlignment.Left
                        Button.onClick (fun _ -> dispatch (AddInputOpened ""))
                    ] :> Avalonia.FuncUI.Types.IView
        ]
    ] :> Avalonia.FuncUI.Types.IView
