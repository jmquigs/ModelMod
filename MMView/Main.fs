module Main

open System
open System.IO
open System.Collections.Generic
open Microsoft.Xna.Framework
open Microsoft.Xna.Framework.Input
open Microsoft.Xna.Framework.Graphics

open ModelMod
open ModelMod.MMView
open ModelMod.CoreTypes

// TODO:
// Camera
//  - fixed distance and position
//  -  shift/middle mouse pan (x/y)
//  -  middle: rotate object 
//  -  roll middle: zoom
//  - rotates object
//  - zoom/pan accomplished with fov changes (Perspective) or size changes (Ortho)
//  
// InputProducer
// Mesh
// IMeshRenderer
//  XNAMeshRenderer

let log = Logging.GetLogger("MMViewMain")

type World = { 
    RenderObjects: XnaRender.IXnaRenderable list 
    Camera: Camera.CameraData
    ModDB: ModDB.ModDB
}

let defaultPosition = Vector3(0.f, 0.f, 80.f)

let mutable world = {
    RenderObjects=[];
    Camera = Camera.make Matrix.Identity defaultPosition
    ModDB = new ModDB.ModDB([],[],[])
}

let resetCamera (viewport:Viewport) position =
    world <- 
    { 
        world with 
            Camera = Camera.make 
                (Matrix.CreatePerspectiveFieldOfView(
                        float32 (System.Math.PI / 4.0), 
                        (float32 viewport.Width / float32 viewport.Height), 
                        0.0001f, 
                        1000.0f))
                position
    }
        
let setModDB db = 
    world <-
    {
        world with ModDB = db
    }

let addToWorld(o) =
    let currObjs = world.RenderObjects
    let found = currObjs |> List.exists (fun x -> x = o) 
    if not found then
        world <- { world with RenderObjects = List.Cons (o, currObjs) }

type MeshViewApp(conf) as self =
    inherit Game()

    let _graphics = new GraphicsDeviceManager(self)
    
    let _conf = conf

    override m.Initialize() =
        base.Initialize()
        
    override m.LoadContent() =
        base.LoadContent()

        resetCamera(self.GraphicsDevice.Viewport) defaultPosition

        match conf.AppSettings with 
        | None -> ()
        | Some settings ->
            match settings.Window with
            | None -> ()
            | Some ws ->                
                self.Window.AllowUserResizing <- ws.AllowResize
                self.Window.Position <- new Point(ws.PosX,ws.PosY)
                _graphics.PreferredBackBufferWidth <- ws.Width
                _graphics.PreferredBackBufferHeight <- ws.Height
                _graphics.ApplyChanges()

                resetCamera(self.GraphicsDevice.Viewport) defaultPosition

            match settings.CamPosition with
            | None -> ()
            | Some position -> 
                resetCamera(self.GraphicsDevice.Viewport) position

        let moddb = ModDB.LoadModDB(_conf)
        setModDB moddb

        for ref in moddb.References do
            addToWorld(XnaRender.MakeMesh(self.GraphicsDevice, ref.Mesh))
            
    override m.UnloadContent() =
        base.UnloadContent()

    override m.Update(gameTime) =

        let keyState = Keyboard.GetState()

        let newCamera = Camera.update world.Camera keyState gameTime

        world <- { world with Camera = newCamera }

        base.Update(gameTime)
        
    override m.Draw(gameTime) =

        base.GraphicsDevice.Clear(Color.Indigo) 
        
        let wrd = { XnaRender.WorldRenderData.Device = base.GraphicsDevice; 
                    XnaRender.WorldRenderData.View = world.Camera.View; 
                    XnaRender.WorldRenderData.Projection = world.Camera.Projection }
        for o in world.RenderObjects do
            o.Update gameTime.ElapsedGameTime.Milliseconds
            o.Render wrd

        base.Draw gameTime

let findFilePath basename =
    let rec findIt walk =
        if walk = null || not (Directory.Exists(walk)) then None
        else
            let path = Path.Combine(walk,basename)
            if File.Exists(path) then Some path
            else 
                let parent = Directory.GetParent(walk)
                let parent = if parent <> null then parent.ToString() else null
                findIt parent

    findIt (Directory.GetCurrentDirectory())
    
let parseCommandLine (argv:string[]) = 
    if argv.Length = 0 then
        None,None
    else
        let mutable argIdx = 0
        let mutable fileToLoad = ""
        let mutable winSettings = None
        let mutable transform = true
        let mutable camPos = None

        let (|FileToLoad|_|) (optName:string, _:string option) = if not (optName.StartsWith("-")) then Some(optName,1) else None

        let (|Transform|_|) (optName:string, optValue:string option) =
            let optName = optName.ToLowerInvariant()
            if optName <> "-transform" then None
            else
                let illegalMessage = "-transform must be followed by true or false (default true)"
                match optValue with
                | None -> failwith illegalMessage
                | Some s -> Some(Convert.ToBoolean(s),2)

        let (|CamPos|_|) (optName:string, optValue:string option) =
            let optName = optName.ToLowerInvariant()
            if optName <> "-campos" then None
            else
                let illegalMessage = "-campos must be followed by: X,Y,Zed"
                match optValue with
                | None -> failwith illegalMessage
                | Some s -> 
                    let parts = s.Split(',')
                    if parts.Length <> 3 then failwith illegalMessage
                    else   
                        Some (Vec3F(float32 parts.[0], float32 parts.[1], float32 parts.[2]), 2)

        let (|WinSettings|_|) (optName:string, optValue:string option) =
            let optName = optName.ToLowerInvariant()
            if optName <> "-win" then None
            else
                let illegalMessage = "-win must be followed by settings: posX,posY,width,height"
                match optValue with
                | None -> failwith illegalMessage
                | Some s -> 
                    let parts = s.Split(',')
                    if parts.Length <> 4 then failwith illegalMessage
                    else   
                        let winSet = { 
                            MMView.WinSettings.PosX = int parts.[0]
                            PosY = int parts.[1]
                            Width = int parts.[2]
                            Height = int parts.[3]
                            AllowResize = false
                        }
                        Some (winSet,2)

        let arg idx =
            if idx >= argv.Length then None
            else Some (argv.[idx])

        while argIdx < argv.Length do
            let adv = 
                match argv.[argIdx],arg (argIdx + 1) with
                | WinSettings (winSet,adv) -> 
                    winSettings <- Some (winSet)
                    adv
                | Transform (trans,adv) ->
                    transform <- trans
                    adv
                | CamPos (pos,adv) ->
                    camPos <- Some pos
                    adv
                | FileToLoad (s,adv) -> 
                    if not (File.Exists(s)) then failwithf "File does not exist: %s" s
                    fileToLoad <- s
                    adv
                | a,b -> failwithf "Unrecognized command line option: %A %A" a b
            argIdx <- argIdx + adv

        Some (fileToLoad), Some 
            ({ 
                AppSettings.Window = winSettings; 
                Transform = transform ;
                CamPosition = camPos
            })
    
let run(argv:string[]) =
    log.Info "wd: %A" (Directory.GetCurrentDirectory())
    log.Info "args: %A" argv

    // use command line args, if present, to create app settings, and specify an optional file to load directly
    let fileToLoad,appSettings = parseCommandLine(argv)
    
    let loadConfWithSettings confPath appSettings = 
        let conf = MMView.loadConf confPath None
        match conf with
        | None -> failwithf "Failed to load conf file: %s" confPath
        | Some conf ->     
            { conf with AppSettings = appSettings }

    let conf = 
        match fileToLoad with 
        | None -> // look for default MMView.yaml                
            let confFile = "MMView.yaml"
            let confPath = findFilePath confFile
            match confPath with 
            | None -> failwithf "Cannot find %s in %s or any parent directory" confFile (Directory.GetCurrentDirectory())
            | Some path -> loadConfWithSettings path appSettings
        | Some file when Path.GetExtension(file).ToLowerInvariant().Trim().Equals(".mmobj") ->
            // direct load of mesh file
            { 
                ModIndexFile = None
                MMView.Conf.FilesToLoad = [ file ] 
                AppSettings = appSettings
            }
        | Some file when Path.GetExtension(file).ToLowerInvariant().Trim().Equals(".yaml") ->
            // alternate conf file path
            loadConfWithSettings file appSettings
        | Some unknownFile -> failwithf "Unknown load file type: %s" unknownFile

    use game = new MeshViewApp(conf)
    game.Run()
