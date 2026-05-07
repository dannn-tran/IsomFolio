module IsomFolio.UI.ContextMenuExt

open System.Runtime.InteropServices
open Avalonia.Controls
open Avalonia.Input
open Avalonia.Interactivity
open Avalonia.FuncUI.DSL

// --- DSL types (mirrors Avalonia.FuncUI MenuItem / ContextMenu attr types) ---

type XMenuItemAttr =
    | Header of string
    | OnClick of (RoutedEventArgs -> unit)
    | SubItems of XMenuItemAttr list list

type XContextMenuAttr =
    | Items of XMenuItemAttr list list

// --- MenuItem DSL (mirrors MenuItem module) ---

module XMenuItem =
    let create (attrs: XMenuItemAttr list) : XMenuItemAttr list = attrs
    let header (text: string)                                   = Header text
    let onClick (handler: RoutedEventArgs -> unit)              = OnClick handler
    let subItems (items: XMenuItemAttr list list)               = SubItems items

// --- ContextMenu DSL (mirrors ContextMenu module) ---

module XContextMenu =
    let create (attrs: XContextMenuAttr list) : XContextMenuAttr list = attrs
    let viewItems (items: XMenuItemAttr list list)                     = Items items

// --- Platform detection ---

let private isContextMenuTrigger (e: PointerPressedEventArgs) =
    let props = e.GetCurrentPoint(null).Properties
    props.PointerUpdateKind = PointerUpdateKind.RightButtonPressed ||
    (RuntimeInformation.IsOSPlatform(OSPlatform.OSX) &&
     props.IsLeftButtonPressed &&
     e.KeyModifiers.HasFlag(KeyModifiers.Control))

// --- Materialization ---

let rec private buildMenuItem (itemDef: XMenuItemAttr list) : MenuItem =
    let item = MenuItem()
    for ia in itemDef do
        match ia with
        | Header h        -> item.Header <- h
        | OnClick handler -> item.Click.Add(handler)
        | SubItems subDefs ->
            for subDef in subDefs do
                item.Items.Add(buildMenuItem subDef) |> ignore
    item

let private openMenu (def: XContextMenuAttr list) (source: obj) =
    let menu = ContextMenu()
    for attr in def do
        match attr with
        | Items itemDefs ->
            for itemDef in itemDefs do
                menu.Items.Add(buildMenuItem itemDef) |> ignore
    match source with
    | :? Control as c -> menu.Open(c)
    | _ -> ()

let private handle (def: XContextMenuAttr list) (e: PointerPressedEventArgs) =
    if isContextMenuTrigger e then
        e.Handled <- true
        openMenu def e.Source

let private handleLeftClick (def: XContextMenuAttr list) (e: PointerPressedEventArgs) =
    let props = e.GetCurrentPoint(null).Properties
    if props.PointerUpdateKind = PointerUpdateKind.LeftButtonPressed then
        e.Handled <- true
        openMenu def e.Source

// --- Per-control attrs (mirrors Border.contextMenu / StackPanel.contextMenu) ---

module XBorder =
    let contextMenu def =
        Border.onPointerPressed((fun e -> handle def e), SubPatchOptions.Always)
    let dropdownMenu def =
        Border.onPointerPressed((fun e -> handleLeftClick def e), SubPatchOptions.Always)

module XStackPanel =
    let contextMenu def =
        StackPanel.onPointerPressed((fun e -> handle def e), SubPatchOptions.Always)
