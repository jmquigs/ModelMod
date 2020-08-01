// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU Lesser General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

namespace MMLaunch

open System.IO

open WpfInteropSample
open ModelMod.StartConf
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

        if (File.Exists selectedFile) then
            let conf = { 
                Conf.ModIndexFile = None
                FilesToLoad = [selectedFile]
                AppSettings = 
                    Some({
                            Window = None
                            CamPosition = Some(Vec3F(0.f,3.75f,10.0f))
                            MeshReadFlags = { ReadMaterialFile = true; ReverseTransform = false }
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