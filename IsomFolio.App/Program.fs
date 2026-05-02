namespace IsomFolio

open Avalonia
open Avalonia.Controls
open Avalonia.Themes.Fluent
open Avalonia.FuncUI.Hosts
open Avalonia.FuncUI.Elmish
open Avalonia.Controls.ApplicationLifetimes

type MainWindow() as this =
    inherit HostWindow()
    do
        base.Title <- "IsomFolio"
        base.Width <- 1400.0
        base.Height <- 900.0
        base.MinWidth <- 800.0
        base.MinHeight <- 600.0

        Elmish.Program.mkProgram (UI.MainView.init (this :> Window)) UI.MainView.update UI.MainView.view
        |> Program.withHost this
        |> Program.runWithAvaloniaSyncDispatch ()  // ensures that when Cmd.OfAsync or Cmd.OfTask finishes and tries to send a message back to the application, that message is executed on the main UI thread, ensuring the view refreshes properly.

type App() =
    inherit Application()

    override this.Initialize() =
        this.Styles.Add(FluentTheme())
        this.RequestedThemeVariant <- Styling.ThemeVariant.Dark

    override this.OnFrameworkInitializationCompleted() =
        match this.ApplicationLifetime with
        | :? IClassicDesktopStyleApplicationLifetime as desktopLifetime ->
            desktopLifetime.MainWindow <- MainWindow()
        | _ -> ()

module Program =

    [<EntryPoint>]
    let main (args: string[]) =
        AppBuilder
            .Configure<App>()
            .UsePlatformDetect()
            .UseSkia()
            .StartWithClassicDesktopLifetime(args)
