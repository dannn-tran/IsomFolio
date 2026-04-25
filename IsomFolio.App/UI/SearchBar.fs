module IsomFolio.UI.SearchBar

open Avalonia.FuncUI.DSL
open Avalonia.Controls
open Avalonia.Layout

type State = { InputText: string }

type Msg =
    | TextChanged   of string
    | QuerySubmitted of string   // fired after 300ms debounce

let init () = { InputText = "" }

let update (msg: Msg) (state: State) =
    match msg with
    | TextChanged t   -> { state with InputText = t }
    | QuerySubmitted _ -> state

let mutable private debounceTimer: System.Threading.Timer option = None
let private debounceMs = 300

let view (state: State) (dispatch: Msg -> unit) =
    TextBox.create [
        TextBox.watermark "Search files and tags…"
        TextBox.text state.InputText
        TextBox.onTextChanged (fun t ->
            dispatch (TextChanged t)
            debounceTimer |> Option.iter (fun tmr -> tmr.Dispose())
            debounceTimer <-
                Some(new System.Threading.Timer(
                    (fun _ ->
                        Avalonia.Threading.Dispatcher.UIThread.Post(fun () ->
                            dispatch (QuerySubmitted t))),
                    null, debounceMs, System.Threading.Timeout.Infinite)))
        TextBox.horizontalAlignment HorizontalAlignment.Stretch
    ]
