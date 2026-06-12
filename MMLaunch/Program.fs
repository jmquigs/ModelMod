namespace MMLaunch

open System
open Avalonia

module Program =
    let buildAvaloniaApp () : AppBuilder =
        AppBuilder
            .Configure<App>()
            .UsePlatformDetect()
            // Avalonia 11 can present a blank first frame on the GPU compositor
            // path until the window receives input/resize (the "blank until you
            // press Tab" bug). Software rendering composites synchronously and
            // blits the frame, sidestepping that race; fine for this lightweight
            // launcher UI.
            .With(Win32PlatformOptions(RenderingMode = [| Win32RenderingMode.Software |]))
            .WithInterFont()

    [<STAThread>]
    [<EntryPoint>]
    let main argv =
        let builder = buildAvaloniaApp ()
        builder.StartWithClassicDesktopLifetime(argv)
