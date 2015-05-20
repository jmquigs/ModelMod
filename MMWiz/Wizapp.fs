namespace MMWiz

open System.Windows.Forms
open System.Diagnostics
open System.IO

open YamlDotNet.RepresentationModel
open YamlDotNet.Serialization

type WizappState = {
    ModRoot: string
    SnapshotRoot: string
    PreviewProcess: Process option
    SelectedSourceFile: string option
    MainScreenForm: WizUI.MainScreen option
    CreateModForm:  WizUI.MakeModForm option
}

module Wizapp =
    let private state = 
        ref 
            ({
                MainScreenForm = None
                CreateModForm = None
                ModRoot = ""
                SnapshotRoot = ""
                PreviewProcess = None
                SelectedSourceFile = None
            })

    let updateState newState = state := newState //; printfn "new state: %A" state.Value

    let setDirectories(modRoot,snapshotRoot) = 
        updateState 
            { state.Value with
                ModRoot = modRoot
                SnapshotRoot = snapshotRoot
            }

    let runProc (proc:Process) (acceptableReturnCode:int) = 
        printfn "running %s %s" proc.StartInfo.FileName proc.StartInfo.Arguments
        let res = proc.Start()
        if not res then
            failwithf "Failed to start backup process: %s %s; code: %d" proc.StartInfo.FileName proc.StartInfo.Arguments proc.ExitCode

    let errDialog (s:string) = MessageBox.Show(s, "ER-ROAR") |> ignore

    let okDialog (s:string) = MessageBox.Show(s, "Click OK") |> ignore

    let terminatePreviewProcess() = 
        match state.Value.PreviewProcess with 
        | None -> ()
        | Some proc ->
            printfn "terminating PP %A" proc
            if not proc.HasExited then
                proc.Kill()
                proc.WaitForExit()      
        updateState { state.Value with PreviewProcess = None }

    let opendlg(form:Form) = 
        let dlg = new OpenFileDialog()

        dlg.InitialDirectory <- state.Value.SnapshotRoot

        dlg.Filter <- "MMObj files (*.mmobj)|*.mmobj"
        dlg.FilterIndex <- 0
        dlg.RestoreDirectory <- true

        let res = dlg.ShowDialog() 
        match res with 
        | DialogResult.OK ->
            //printfn "%A" dlg.FileName
            terminatePreviewProcess()

            let proc = new Process()
            updateState { state.Value with PreviewProcess = Some proc }
        
            proc.StartInfo.UseShellExecute <- false
            proc.StartInfo.FileName <- @"C:\Dev\modelmod.new\MMView\bin\Release\MeshView.exe"
            let winSettings = sprintf "%d,%d,%d,%d" (form.Location.X + form.Size.Width) (form.Location.Y) (500) (500) //square preview window
            proc.StartInfo.Arguments <- (sprintf "\"%s\" -transform false -campos 0.0,3.75,10.0 -win %s" dlg.FileName winSettings)       
            runProc proc 0

            Some (dlg.FileName)
        | _ -> 
            None

    let getOutputPath modName = Path.GetFullPath(Path.Combine(state.Value.ModRoot, modName))

    type YamlRef = {
        Type: string
        MeshPath: string
        VertDeclPath: string
        ExpectedPrimCount: int
        ExpectedVertCount: int
    }

    type YamlMod = {
        Type: string
        Ref: string
        MeshType: string
        MeshPath: string
    }
        
    let createMod (modName:string) (srcMMObjFile:string) = 
        try
            let modName = modName.Trim()
            if modName = "" then 
                failwith "Please enter a mod name"

            if (not (File.Exists srcMMObjFile)) then 
                failwith "Please verify that the source file exists"
        
            let modOutDir = getOutputPath modName
            if (Directory.Exists modOutDir) then 
                failwithf "Mod directory already exists, try a different mod name? dir: %s" modOutDir

            // make sure filename conforms to expected format
            let (|SnapshotFilename|_|) pattern str = 
                ModelMod.REUtil.CheckGroupMatch pattern 4 str 
                |> ModelMod.REUtil.Extract 1 int32 
                |> (fun (intParts: int [] option) ->
                        match intParts with
                        | None -> None
                        | Some parts -> // omit snapshot number but return primcount, vertcount
                            Some (parts.[1],parts.[2]))

            let pCount,vCount = 
                match srcMMObjFile.ToLowerInvariant() with 
                | SnapshotFilename @"snap_(\S+)_(\d+)p_(\d+)v.*" parts -> parts
                | _ -> failwithf "Illegal snapshot filename; cannot build a ref from it: %s" srcMMObjFile

            let srcBasename = Path.GetFileNameWithoutExtension(srcMMObjFile)
            let snapSrcDir = Path.GetDirectoryName(srcMMObjFile)
            let refBasename = modName + "Ref"
            let modBasename = modName + "Mod"

            Directory.CreateDirectory modOutDir |> ignore

            // copy vb declaration
            let vbDeclFile =
                let declExt = ".dat"
                let declSuffix = "_VBDecl"
                let declSrc = Path.Combine(snapSrcDir, srcBasename + declSuffix + declExt)
                if not (File.Exists(declSrc)) then failwithf "No decl source file found; it is required: %s" declSrc
                else
                    let newDeclFile = Path.Combine(modOutDir, refBasename + declSuffix + declExt)
                    File.Copy(declSrc,newDeclFile)
                    newDeclFile

            // copy texture file
            let texFile = 
                let texExt = ".dds"
                let texSuffix = "_texture0"
                let texSrc = Path.Combine(snapSrcDir, srcBasename + texSuffix + texExt)
                if not (File.Exists(texSrc)) then None
                else
                    let newTexFile = Path.Combine(modOutDir, refBasename + texExt)
                    File.Copy(texSrc,newTexFile)
                    Some newTexFile

            // copy mtl file and rename texture
            let mtlFile = 
                let mtlExt = ".mtl"
                let mtlSrc = Path.Combine(snapSrcDir, srcBasename + mtlExt)
                if not (File.Exists(mtlSrc)) then None
                else
                    let newMtlFile = Path.Combine(modOutDir, refBasename + mtlExt)
                    let fDat = File.ReadAllLines(mtlSrc)
                    let fDat = fDat |> Array.map (fun line -> 
                        match line with
                        | l when texFile <> None && l.StartsWith("map_Kd ") -> "map_Kd " + Path.GetFileName(Option.get texFile)
                        | l -> l)                            
                    File.WriteAllLines(newMtlFile, fDat)
                    Some newMtlFile

            // copy mmobj and rename mtl
            let refMMObjFile = 
                // we already checked that the src exists
                let newMMObjFile = Path.Combine(modOutDir, refBasename + ".mmobj")
                let fDat = File.ReadAllLines(srcMMObjFile)
                let fDat = fDat |> Array.map (fun line ->
                    match line with
                    | l when mtlFile <> None && l.StartsWith("mtllib ") -> "mtllib " +  Path.GetFileName(Option.get mtlFile)
                    | l when l.StartsWith("o ") -> "o " + modName 
                    | l -> l)
                File.WriteAllLines(newMMObjFile, fDat)
                newMMObjFile

            // generate a default mod file that is a copy of the ref
            let modMMObjFile = 
                let modMMObjFile = Path.Combine(modOutDir, modBasename + ".mmobj")
                File.Copy(refMMObjFile,modMMObjFile)
                modMMObjFile

            // generate ref yaml
            let refYamlFile = 
                let refYamlFile = Path.Combine(modOutDir, refBasename + ".yaml")
                let refObj = {
                    YamlRef.Type = "Reference"
                    YamlRef.MeshPath = Path.GetFileName(refMMObjFile)
                    YamlRef.VertDeclPath = Path.GetFileName(vbDeclFile)
                    YamlRef.ExpectedPrimCount = pCount
                    YamlRef.ExpectedVertCount = vCount
                }
                let sr = new Serializer()
                use sw = new StreamWriter(refYamlFile)
                sr.Serialize(sw, refObj) 
                refYamlFile

            // generate mod yaml 
            let modYamlFile = 
                let modYamlFile = Path.Combine(modOutDir, modBasename + ".yaml")
                let modObj = { 
                    YamlMod.Type = "Mod"
                    MeshType = "GPUReplacement"
                    Ref = refBasename
                    MeshPath = Path.GetFileName(modMMObjFile)
                }
                let sr = new Serializer()
                use sw = new StreamWriter(modYamlFile)
                sr.Serialize(sw, modObj) 
                modYamlFile

            okDialog(sprintf "Mod created: import %s into blender to get started" modMMObjFile)
        with 
            | e -> errDialog(e.Message)

    let initMakeModForm() =
        let frm = new WizUI.MakeModForm()

        frm.OpenBTN.Click.Add (fun (evArgs) -> 
            let selFile = opendlg(frm)
            updateState { state.Value with SelectedSourceFile = selFile  }
        )

        let emptyTargetDirText = "<Enter Mod name>"
        frm.ModNameTB.TextChanged.Add (fun (evArgs) -> 
            //printfn "%s %s" nameTxt.Text (getOutputPath nameTxt.Text)
            let modName = frm.ModNameTB.Text
            if modName = "" then
                frm.TargetDirLbl.Text <- emptyTargetDirText
            else
                frm.TargetDirLbl.Text <- getOutputPath modName
        )
        frm.TargetDirLbl.Text <- emptyTargetDirText

        frm.CreateButton.Click.Add (fun (evArgs) ->
            match state.Value.SelectedSourceFile with
                | None -> errDialog("Please select a source MMObj file")
                | Some path -> createMod (frm.ModNameTB.Text.Trim()) (path))

        frm.Closed.Add (fun (evArgs) -> 
            terminatePreviewProcess() )

        frm

    let initMainScreen() =
        let ms = new WizUI.MainScreen()
        ms.btnCreateMod.Click.Add(fun (evArgs) ->
            match state.Value.CreateModForm with 
            | None -> ()
            | Some form -> form.Close()
            let frm = initMakeModForm()
            updateState { state.Value with CreateModForm = Some frm }
            frm.Show(ms)
        )

        let updateSelectedProfile() = 
            // TODO: btn create mod should be here too; do this after I've added the profile logic
            let requireActive = [ms.btnStartPlayback; ms.btnStartSnap; ms.btnDeleteProfile]

            match ms.lbProfiles.SelectedItem with 
            | null -> 
                requireActive |> List.iter (fun ctrl -> ctrl.Enabled <- false)
            | profile -> 
                requireActive |> List.iter (fun ctrl -> ctrl.Enabled <- true)
            ()

        updateSelectedProfile()

        updateState { state.Value with MainScreenForm = Some ms }
        ms

//    let stuff() = 
//        let sr = new Serializer()
//        let sw = new StringWriter()
//        //sr.Serialize(sw, new YRef(@"AAA \BBB.mmobj", @"AAA \BBB.dat"))        
//        printfn "%s" (sw.ToString())
            
//        let x = new YamlMappingNode()
//        x.Add("type:", "Reference")
//        printfn "%A" <| x.ToString()
    //do 
    //    showForm() |> ignore
