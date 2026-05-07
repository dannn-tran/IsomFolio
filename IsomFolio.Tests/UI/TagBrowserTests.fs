module IsomFolio.App.Tests.UI.TagBrowserTests

open Xunit
open IsomFolio.UI.TagBrowser

let private withTags tags = { init () with AllTags = tags }


module FilterChanged =

    [<Fact>]
    let ``stores filter text`` () =
        let state = init () |> update (FilterChanged "beach")
        Assert.Equal("beach", state.Filter)

    [<Fact>]
    let ``filteredTags returns all when filter is blank`` () =
        let state = withTags [ "beach", 5; "travel", 3 ]
        Assert.Equal<(string * int) list>([ "beach", 5; "travel", 3 ], filteredTags state)

    [<Fact>]
    let ``filteredTags applies case-insensitive substring match`` () =
        let state =
            withTags [ "beach", 5; "beachside", 2; "travel", 3 ]
            |> update (FilterChanged "Beach")
        Assert.Equal<(string * int) list>([ "beach", 5; "beachside", 2 ], filteredTags state)

    [<Fact>]
    let ``filteredTags returns empty list when nothing matches`` () =
        let state =
            withTags [ "beach", 5; "travel", 3 ]
            |> update (FilterChanged "xyz")
        Assert.Empty(filteredTags state)


module Rename =

    [<Fact>]
    let ``RenameStarted sets RenameInput with tag as initial text`` () =
        let state = init () |> update (RenameStarted "beach")
        Assert.Equal(Some ("beach", "beach"), state.RenameInput)

    [<Fact>]
    let ``RenameStarted clears PendingDelete`` () =
        let state =
            init ()
            |> update (DeleteArmed "beach")
            |> update (RenameStarted "travel")
        Assert.True(state.PendingDelete.IsNone)

    [<Fact>]
    let ``RenameTextChanged updates current text`` () =
        let state =
            init ()
            |> update (RenameStarted "beach")
            |> update (RenameTextChanged "seaside")
        Assert.Equal(Some ("beach", "seaside"), state.RenameInput)

    [<Fact>]
    let ``RenameTextChanged no-op when no RenameInput`` () =
        let state = init () |> update (RenameTextChanged "seaside")
        Assert.True(state.RenameInput.IsNone)

    [<Fact>]
    let ``RenameCancelled clears RenameInput`` () =
        let state =
            init ()
            |> update (RenameStarted "beach")
            |> update RenameCancelled
        Assert.True(state.RenameInput.IsNone)

    [<Fact>]
    let ``RenameSubmitted is a no-op in pure update (handled in MainView)`` () =
        let before = init () |> update (RenameStarted "beach")
        let after = before |> update RenameSubmitted
        Assert.Equal(before.RenameInput, after.RenameInput)


module Delete =

    [<Fact>]
    let ``DeleteArmed sets PendingDelete`` () =
        let state = init () |> update (DeleteArmed "beach")
        Assert.Equal(Some "beach", state.PendingDelete)

    [<Fact>]
    let ``DeleteArmed clears RenameInput`` () =
        let state =
            init ()
            |> update (RenameStarted "beach")
            |> update (DeleteArmed "travel")
        Assert.True(state.RenameInput.IsNone)

    [<Fact>]
    let ``DeleteCancelled clears PendingDelete`` () =
        let state =
            init ()
            |> update (DeleteArmed "beach")
            |> update DeleteCancelled
        Assert.True(state.PendingDelete.IsNone)

    [<Fact>]
    let ``DeleteConfirmed is a no-op in pure update (handled in MainView)`` () =
        let before = init () |> update (DeleteArmed "beach")
        let after = before |> update DeleteConfirmed
        Assert.Equal(before.PendingDelete, after.PendingDelete)


module MutationCompleted =

    [<Fact>]
    let ``updates AllTags`` () =
        let state = withTags [ "beach", 5 ] |> update (MutationCompleted [ "seaside", 5 ])
        Assert.Equal<(string * int) list>([ "seaside", 5 ], state.AllTags)

    [<Fact>]
    let ``clears RenameInput`` () =
        let state =
            withTags [ "beach", 5 ]
            |> update (RenameStarted "beach")
            |> update (MutationCompleted [ "seaside", 5 ])
        Assert.True(state.RenameInput.IsNone)

    [<Fact>]
    let ``clears PendingDelete`` () =
        let state =
            withTags [ "beach", 5 ]
            |> update (DeleteArmed "beach")
            |> update (MutationCompleted [])
        Assert.True(state.PendingDelete.IsNone)
