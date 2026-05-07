module IsomFolio.Tests.UI.SidebarTests

open Xunit
open IsomFolio.Core.Models
open IsomFolio.UI

let private makeAlbum id name : Album =
    { Id = id; Name = name; Kind = Manual; SortOrder = 0 }

module AlbumState =

    [<Fact>]
    let ``AlbumsLoaded updates Albums`` () =
        let albums = [ makeAlbum "a1" "Tokyo"; makeAlbum "a2" "Paris" ]
        let next = Sidebar.update (Sidebar.AlbumsLoaded albums) (Sidebar.init ())
        Assert.Equal<Album list>(albums, next.Albums)

    [<Fact>]
    let ``AlbumSelected sets SelectedAlbumId`` () =
        let state = { Sidebar.init () with Albums = [ makeAlbum "a1" "Tokyo" ] }
        let next = Sidebar.update (Sidebar.AlbumSelected "a1") state
        Assert.Equal(Some "a1", next.SelectedAlbumId)

    [<Fact>]
    let ``AlbumSelected clears SelectedFolder`` () =
        let state = { Sidebar.init () with SelectedFolder = Some "/photos" }
        let next = Sidebar.update (Sidebar.AlbumSelected "a1") state
        Assert.Equal(None, next.SelectedFolder)

    [<Fact>]
    let ``AlbumDeselected clears SelectedAlbumId`` () =
        let state = { Sidebar.init () with SelectedAlbumId = Some "a1" }
        let next = Sidebar.update Sidebar.AlbumDeselected state
        Assert.Equal(None, next.SelectedAlbumId)

    [<Fact>]
    let ``FolderSelected clears SelectedAlbumId`` () =
        let state = { Sidebar.init () with SelectedAlbumId = Some "a1" }
        let next = Sidebar.update (Sidebar.FolderSelected "/photos") state
        Assert.Equal(None, next.SelectedAlbumId)
        Assert.Equal(Some "/photos", next.SelectedFolder)

    [<Fact>]
    let ``AlbumCreateRequested is pass-through`` () =
        let state = { Sidebar.init () with Albums = [ makeAlbum "a1" "Tokyo" ] }
        let next = Sidebar.update Sidebar.AlbumCreateRequested state
        Assert.Equal<Album list>(state.Albums, next.Albums)

    [<Fact>]
    let ``AlbumRenameRequested is pass-through`` () =
        let state = { Sidebar.init () with SelectedAlbumId = Some "a1" }
        let next = Sidebar.update (Sidebar.AlbumRenameRequested "a1") state
        Assert.Equal(state.SelectedAlbumId, next.SelectedAlbumId)

    [<Fact>]
    let ``AlbumDeleteRequested is pass-through`` () =
        let state = { Sidebar.init () with SelectedAlbumId = Some "a1" }
        let next = Sidebar.update (Sidebar.AlbumDeleteRequested "a1") state
        Assert.Equal(state.SelectedAlbumId, next.SelectedAlbumId)
