module IsomFolio.App.Tests.UI.TagTreeTests

open Xunit
open IsomFolio.Core.Metadata.TagTree
open IsomFolio.UI.TagTree

let private paths (state: State) =
    flattenTree state.Roots |> List.sort


module TagAdded =

    [<Fact>]
    let ``adds new tag to empty tree`` () =
        let state = init () |> update (TagAdded "beach")
        Assert.Equal<string list>([ "beach" ], paths state)

    [<Fact>]
    let ``adds hierarchical tag`` () =
        let state = init () |> update (TagAdded "person/John")
        Assert.Equal<string list>([ "person/John" ], paths state)

    [<Fact>]
    let ``new tag path added to Expanded`` () =
        let state = init () |> update (TagAdded "beach")
        Assert.Contains("beach", state.Expanded)

    [<Fact>]
    let ``clears AddInput`` () =
        let state =
            { init () with AddInput = Some ("", "beach") }
            |> update (TagAdded "beach")
        Assert.True(state.AddInput.IsNone)


module TagRemoved =

    [<Fact>]
    let ``removes leaf node`` () =
        let state =
            fromTagList [ "beach"; "travel" ]
            |> update (TagRemoved "beach")
        Assert.Equal<string list>([ "travel" ], paths state)

    [<Fact>]
    let ``demotes tagged parent to Ghost, children survive`` () =
        let state =
            fromTagList [ "person"; "person/John" ]
            |> update (TagRemoved "person")
        Assert.Equal<string list>([ "person/John" ], paths state)
        let person = state.Roots.[0]
        Assert.Equal(Ghost, person.Kind)

    [<Fact>]
    let ``removing only child cleans up Ghost parent`` () =
        let state =
            fromTagList [ "person/John" ]
            |> update (TagRemoved "person/John")
        Assert.Empty(state.Roots)


module SubtreeRemove =

    [<Fact>]
    let ``armed sets PendingRemoveSubtree, tree unchanged`` () =
        let before = fromTagList [ "person"; "person/John" ]
        let after  = before |> update (SubtreeRemoveArmed "person")
        Assert.Equal(Some "person", after.PendingRemoveSubtree)
        Assert.Equal<string list>(paths before, paths after)

    [<Fact>]
    let ``confirmed removes entire subtree`` () =
        let state =
            fromTagList [ "person"; "person/John"; "travel" ]
            |> update (SubtreeRemoveArmed "person")
            |> update SubtreeRemoveConfirmed
        Assert.Equal<string list>([ "travel" ], paths state)
        Assert.True(state.PendingRemoveSubtree.IsNone)

    [<Fact>]
    let ``cancelled clears PendingRemoveSubtree, tree unchanged`` () =
        let before = fromTagList [ "person"; "person/John" ]
        let after =
            before
            |> update (SubtreeRemoveArmed "person")
            |> update SubtreeRemoveCancelled
        Assert.True(after.PendingRemoveSubtree.IsNone)
        Assert.Equal<string list>(paths before, paths after)


module TagRetagged =

    [<Fact>]
    let ``Ghost node becomes Tagged`` () =
        let state =
            fromTagList [ "person/John" ]
            |> update (TagRetagged "person")
        let person = state.Roots.[0]
        Assert.Equal(Tagged, person.Kind)
        Assert.Equal<string list>([ "person"; "person/John" ], paths state)

    [<Fact>]
    let ``no-op when already Tagged`` () =
        let before = fromTagList [ "beach" ]
        let after  = before |> update (TagRetagged "beach")
        Assert.Equal<string list>(paths before, paths after)


module NodeToggled =

    [<Fact>]
    let ``collapses an expanded node`` () =
        let state = fromTagList [ "person/John" ]
        Assert.Contains("person", state.Expanded)
        let collapsed = state |> update (NodeToggled "person")
        Assert.DoesNotContain("person", collapsed.Expanded)

    [<Fact>]
    let ``expands a collapsed node`` () =
        let state = { fromTagList [ "person/John" ] with Expanded = Set.empty }
        let expanded = state |> update (NodeToggled "person")
        Assert.Contains("person", expanded.Expanded)


module AddInput =

    [<Fact>]
    let ``opened sets AddInput with empty text`` () =
        let state = init () |> update (AddInputOpened "")
        Assert.Equal(Some ("", ""), state.AddInput)

    [<Fact>]
    let ``changed updates text`` () =
        let state =
            init ()
            |> update (AddInputOpened "")
            |> update (AddInputChanged "beach")
        Assert.Equal(Some ("", "beach"), state.AddInput)

    [<Fact>]
    let ``submitted at root adds tag and clears input`` () =
        let state =
            init ()
            |> update (AddInputOpened "")
            |> update (AddInputChanged "beach")
            |> update AddInputSubmitted
        Assert.Equal<string list>([ "beach" ], paths state)
        Assert.True(state.AddInput.IsNone)

    [<Fact>]
    let ``submitted under parent prefixes correctly`` () =
        let state =
            fromTagList [ "person/John" ]
            |> update (AddInputOpened "person")
            |> update (AddInputChanged "Jane")
            |> update AddInputSubmitted
        Assert.Contains("person/Jane", paths state)

    [<Fact>]
    let ``submitted with blank text clears input without adding tag`` () =
        let state =
            init ()
            |> update (AddInputOpened "")
            |> update (AddInputChanged "   ")
            |> update AddInputSubmitted
        Assert.Empty(state.Roots)
        Assert.True(state.AddInput.IsNone)

    [<Fact>]
    let ``cancelled clears input without adding tag`` () =
        let state =
            init ()
            |> update (AddInputOpened "")
            |> update (AddInputChanged "beach")
            |> update AddInputCancelled
        Assert.Empty(state.Roots)
        Assert.True(state.AddInput.IsNone)


module AllTagsLoaded =

    [<Fact>]
    let ``stores tag list`` () =
        let state = init () |> update (AllTagsLoaded [ "beach"; "travel" ])
        Assert.Equal<string list>([ "beach"; "travel" ], state.AllTags)

    [<Fact>]
    let ``replaces previous list`` () =
        let state =
            init ()
            |> update (AllTagsLoaded [ "old" ])
            |> update (AllTagsLoaded [ "beach"; "travel" ])
        Assert.Equal<string list>([ "beach"; "travel" ], state.AllTags)


module SuggestionSelected =

    [<Fact>]
    let ``sets AddInput text to selected tag at root`` () =
        let state =
            { init () with AllTags = [ "beach"; "travel" ] }
            |> update (AddInputOpened "")
            |> update (SuggestionSelected "beach")
        Assert.Equal(Some ("", "beach"), state.AddInput)

    [<Fact>]
    let ``strips parent prefix when under a parent`` () =
        let state =
            { init () with AllTags = [ "person/John"; "person/Jane" ] }
            |> update (AddInputOpened "person")
            |> update (SuggestionSelected "person/Jane")
        Assert.Equal(Some ("person", "Jane"), state.AddInput)

    [<Fact>]
    let ``no-op when AddInput is closed`` () =
        let state = init () |> update (SuggestionSelected "beach")
        Assert.True(state.AddInput.IsNone)


module Suggestions =

    [<Fact>]
    let ``returns empty list when AddInput is closed`` () =
        let state = { init () with AllTags = [ "beach" ] }
        Assert.Empty(suggestions state)

    [<Fact>]
    let ``returns empty list when text is blank`` () =
        let state =
            { init () with AllTags = [ "beach" ] }
            |> update (AddInputOpened "")
            |> update (AddInputChanged "  ")
        Assert.Empty(suggestions state)

    [<Fact>]
    let ``filters by prefix match`` () =
        let state =
            { init () with AllTags = [ "beach"; "travel"; "beachside" ] }
            |> update (AddInputOpened "")
            |> update (AddInputChanged "bea")
        Assert.Equal<string list>([ "beach"; "beachside" ], suggestions state)

    [<Fact>]
    let ``excludes already-tagged paths`` () =
        let state =
            { fromTagList [ "beach" ] with AllTags = [ "beach"; "travel" ] }
            |> update (AddInputOpened "")
            |> update (AddInputChanged "bea")
        Assert.Equal<string list>([], suggestions state)

    [<Fact>]
    let ``limits results to 8`` () =
        let tags = [ "a1"; "a2"; "a3"; "a4"; "a5"; "a6"; "a7"; "a8"; "a9"; "a10" ]
        let state =
            { init () with AllTags = tags }
            |> update (AddInputOpened "")
            |> update (AddInputChanged "a")
        Assert.Equal(8, suggestions state |> List.length)

    [<Fact>]
    let ``filters by parent-prefixed path when under a parent`` () =
        let state =
            { init () with AllTags = [ "person/John"; "person/Jane"; "place/Paris" ] }
            |> update (AddInputOpened "person")
            |> update (AddInputChanged "J")
        Assert.Equal<string list>([ "person/John"; "person/Jane" ], suggestions state)
