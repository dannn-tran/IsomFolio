module IsomFolio.UI.MainView

open Avalonia.FuncUI.DSL
open Avalonia.Controls

type State = { Placeholder: unit }
type Msg = | NoOp

let init () = { Placeholder = () }, Elmish.Cmd.none

let update (msg: Msg) (state: State) =
    match msg with
    | NoOp -> state, Elmish.Cmd.none

let view (state: State) (dispatch: Msg -> unit) =
    TextBlock.create [
        TextBlock.text "IsomFolio"
        TextBlock.fontSize 24.0
        TextBlock.horizontalAlignment Avalonia.Layout.HorizontalAlignment.Center
        TextBlock.verticalAlignment Avalonia.Layout.VerticalAlignment.Center
    ] :> Avalonia.FuncUI.Types.IView
