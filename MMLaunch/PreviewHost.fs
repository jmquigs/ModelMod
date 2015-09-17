namespace MMLaunch

open System.IO

open WpfInteropSample
open ModelMod.MMView
open ModelMod.CoreTypes

type PreviewHost() =
    inherit WpfInteropSample.D3D11Host()

    let mutable selectedFile:string = ""
    let mutable control:MeshView.Main.MeshViewControl option = None

    member x.SelectedFile 
        with get() = selectedFile
        and set value = 
            selectedFile <- value
            x.Initialize()

    override x.Initialize() =
        x.Uninitialize()

        if not (File.Exists selectedFile) then
            ()
        else
            let conf = { 
                Conf.ModIndexFile = None
                FilesToLoad = [selectedFile]
                AppSettings = 
                    Some({
                            Window = None
                            Transform = false
                            CamPosition = Some(Vec3F(0.f,3.75f,10.0f))
                    })
            }
            control <- Some(new MeshView.Main.MeshViewControl(conf, x.GraphicsDevice))
        ()
    
    override x.Uninitialize() =
        match control with
        | None -> ()
        | Some ctrl -> (ctrl :> System.IDisposable).Dispose()
        control <- None

    override x.Render(time: System.TimeSpan) = 
        match control with
        | None -> ()
        | Some (control) ->
            
            let gt = new Microsoft.Xna.Framework.GameTime(System.TimeSpan(0L), time)
            control.Update(gt)
            control.Draw(gt)
        ()