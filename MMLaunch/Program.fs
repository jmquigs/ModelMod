namespace MMLaunch

open System
open Avalonia

module Program =
    let buildAvaloniaApp () : AppBuilder =
        AppBuilder
            .Configure<App>()
            .UsePlatformDetect()
            .WithInterFont()

    [<STAThread>]
    [<EntryPoint>]
    let main argv =
        let builder = buildAvaloniaApp ()
        builder.StartWithClassicDesktopLifetime(argv)
