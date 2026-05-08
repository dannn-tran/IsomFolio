module IsomFolio.Tests.UI.SearchBarTests

open System
open Xunit
open IsomFolio.UI.SearchBar

module ParseDateOpt =

    [<Fact>]
    let ``valid date string returns Some`` () =
        let result = parseDateOpt "2024-06-15"
        Assert.Equal(Some (DateTime(2024, 6, 15)), result)

    [<Fact>]
    let ``empty string returns None`` () =
        Assert.Equal(None, parseDateOpt "")

    [<Fact>]
    let ``whitespace-only string returns None`` () =
        Assert.Equal(None, parseDateOpt "   ")

    [<Fact>]
    let ``invalid format returns None`` () =
        Assert.Equal(None, parseDateOpt "15/06/2024")

    [<Fact>]
    let ``partial date returns None`` () =
        Assert.Equal(None, parseDateOpt "2024-06")

module HasCriteria =

    let private fresh () = init ()

    [<Fact>]
    let ``fresh state has no criteria`` () =
        Assert.False(hasCriteria (fresh ()))

    [<Fact>]
    let ``tag filter present returns true`` () =
        let state = { fresh () with TagFilter = [ "travel" ] }
        Assert.True(hasCriteria state)

    [<Fact>]
    let ``date from present returns true`` () =
        let state = { fresh () with DateFrom = "2024-01-01" }
        Assert.True(hasCriteria state)

    [<Fact>]
    let ``date to present returns true`` () =
        let state = { fresh () with DateTo = "2024-12-31" }
        Assert.True(hasCriteria state)

    [<Fact>]
    let ``ext filter present returns true`` () =
        let state = { fresh () with ExtFilter = [ "jpg" ] }
        Assert.True(hasCriteria state)

    [<Fact>]
    let ``folder filter present returns true`` () =
        let state = { fresh () with FolderFilter = Some "/photos" }
        Assert.True(hasCriteria state)

module IsCriteriaMsg =

    [<Fact>]
    let ``TagAdded is criteria`` () =
        Assert.True(isCriteriaMsg (TagAdded "travel"))

    [<Fact>]
    let ``TagRemoved is criteria`` () =
        Assert.True(isCriteriaMsg (TagRemoved "travel"))

    [<Fact>]
    let ``DateFromChanged is criteria`` () =
        Assert.True(isCriteriaMsg (DateFromChanged "2024-01-01"))

    [<Fact>]
    let ``DateToChanged is criteria`` () =
        Assert.True(isCriteriaMsg (DateToChanged "2024-12-31"))

    [<Fact>]
    let ``ExtToggled is criteria`` () =
        Assert.True(isCriteriaMsg (ExtToggled "jpg"))

    [<Fact>]
    let ``FolderFilterSet is criteria`` () =
        Assert.True(isCriteriaMsg (FolderFilterSet (Some "/photos")))

    [<Fact>]
    let ``TextChanged is not criteria`` () =
        Assert.False(isCriteriaMsg (TextChanged "paris"))

    [<Fact>]
    let ``QuerySubmitted is not criteria`` () =
        Assert.False(isCriteriaMsg (QuerySubmitted "paris"))

    [<Fact>]
    let ``CriteriaToggled is not criteria`` () =
        Assert.False(isCriteriaMsg CriteriaToggled)

    [<Fact>]
    let ``SaveAsSmartAlbumRequested is not criteria`` () =
        Assert.False(isCriteriaMsg SaveAsSmartAlbumRequested)

module Update =

    let private fresh () = init ()

    [<Fact>]
    let ``TagAdded appends tag and clears input`` () =
        let state = { fresh () with TagInput = "travel" }
        let next = update (TagAdded "travel") state
        Assert.Contains("travel", next.TagFilter)
        Assert.Equal("", next.TagInput)

    [<Fact>]
    let ``TagAdded ignores duplicate case-insensitively`` () =
        let state = { fresh () with TagFilter = [ "Travel" ]; TagInput = "travel" }
        let next = update (TagAdded "travel") state
        Assert.Equal(1, next.TagFilter.Length)

    [<Fact>]
    let ``TagAdded opens criteria panel`` () =
        let state = { fresh () with TagInput = "travel" }
        let next = update (TagAdded "travel") state
        Assert.True(next.CriteriaOpen)

    [<Fact>]
    let ``TagRemoved removes matching tag`` () =
        let state = { fresh () with TagFilter = [ "travel"; "paris" ] }
        let next = update (TagRemoved "travel") state
        Assert.Equal<string list>([ "paris" ], next.TagFilter)

    [<Fact>]
    let ``ExtToggled adds ext when not present`` () =
        let state = fresh ()
        let next = update (ExtToggled "jpg") state
        Assert.Contains("jpg", next.ExtFilter)

    [<Fact>]
    let ``ExtToggled removes ext when already present`` () =
        let state = { fresh () with ExtFilter = [ "jpg"; "png" ] }
        let next = update (ExtToggled "jpg") state
        Assert.DoesNotContain("jpg", next.ExtFilter)
        Assert.Contains("png", next.ExtFilter)

    [<Fact>]
    let ``FolderFilterSet Some sets folder and opens criteria`` () =
        let state = fresh ()
        let next = update (FolderFilterSet (Some "/photos")) state
        Assert.Equal(Some "/photos", next.FolderFilter)
        Assert.True(next.CriteriaOpen)

    [<Fact>]
    let ``FolderFilterSet None clears folder`` () =
        let state = { fresh () with FolderFilter = Some "/photos" }
        let next = update (FolderFilterSet None) state
        Assert.Equal(None, next.FolderFilter)

    [<Fact>]
    let ``CriteriaToggled flips CriteriaOpen`` () =
        let state = fresh ()
        let next = update CriteriaToggled state
        Assert.True(next.CriteriaOpen)
        let next2 = update CriteriaToggled next
        Assert.False(next2.CriteriaOpen)
