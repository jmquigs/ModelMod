namespace MMWiz

open System.IO
open System.Windows.Forms
open System.Diagnostics

open ModelMod

type WizappState = {
    ModRoot: string
    SnapshotRoot: string
    PreviewProcess: Process option
    SelectedSourceFile: string option
    MainScreenForm: WizUI.MainScreen 
    CreateModForm:  WizUI.MakeModForm option
}

type UIProfile =
    { 
        DisplayName: string
        RegProfileName: string 
        RunConfig: CoreTypes.RunConfig
    }
    override x.ToString() = x.DisplayName

module Wizapp =
    let private EmptyProfile = {
        DisplayName = "<EMPTY>"
        RegProfileName = ""
        RunConfig = CoreTypes.DefaultRunConfig
    }

    let private state = 
        ref 
            ({
                MainScreenForm = new WizUI.MainScreen() //we'll replace this with the real thing later
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

    let uiFail(s:string) = MessageBox.Show("A UI failure has occurred, please report this bug: " + s, "Oops") |> ignore

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
                frm.TargetDirLbl.Text <- ModUtil.getOutputPath state.Value.ModRoot modName
        )
        frm.TargetDirLbl.Text <- emptyTargetDirText

        frm.CreateButton.Click.Add (fun (evArgs) ->
            match state.Value.SelectedSourceFile with
                | None -> errDialog("Please select a source MMObj file")
                | Some path ->  
                    let res = ModUtil.createMod state.Value.ModRoot (frm.ModNameTB.Text.Trim()) (path)
                    match res with 
                    | ModUtil.Success modFile ->                         
                        okDialog(sprintf "Mod created: import %s into blender to get started" modFile)
                    | ModUtil.Error msg -> 
                        errDialog(msg))

        frm.Closed.Add (fun (evArgs) -> 
            terminatePreviewProcess() )

        frm

    let selectExecutableDialog(form:Form) = 
        let dlg = new OpenFileDialog()

        //dlg.InitialDirectory <- state.Value.SnapshotRoot

        dlg.Filter <- "Executable files (*.exe)|*.exe"
        dlg.FilterIndex <- 0
        dlg.RestoreDirectory <- true

        let res = dlg.ShowDialog() 
        match res with 
        | DialogResult.OK ->
            Some (dlg.FileName)
        | _ -> 
            None

    let pendingProfileName = "New Profile"

    let profileLabelFromPath (path:string) =
        let path = path.Trim()
        match path with 
        | "" -> pendingProfileName
        | p -> Path.GetFileNameWithoutExtension(p)
        
    let findLbProfile exePath = 
        // not using IndexOf from the collection here because we don't want an equality check on the full 
        // profile object.  Just want display name.
        let idx = 
            state.Value.MainScreenForm.lbProfiles.Items 
            |> Seq.cast<UIProfile>
            |> Seq.findIndex (fun p -> p.RunConfig.ExePath = exePath)
        idx

    let updateUiProfile (profile:UIProfile,newExePath) = 
            let idx = findLbProfile profile.RunConfig.ExePath
            let uiProfile = state.Value.MainScreenForm.lbProfiles.Items.[idx] :?> UIProfile
            let newLabel = profileLabelFromPath(newExePath)

            let runConfig = {
                profile.RunConfig with
                    ExePath = newExePath
            }
            let profile = 
                { profile with
                    DisplayName = newLabel
                    RunConfig = runConfig
                }

            state.Value.MainScreenForm.lbProfiles.Items.[idx] <- profile

    //let isProfileSelected() = state.Value.MainScreenForm.lbProfiles.SelectedItem <> null
    let getSelectedProfile() = state.Value.MainScreenForm.lbProfiles.SelectedItem 

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

        ms.btnNewProfile.Click.Add(fun (evArgs) ->
            // make sure current profile is valid before doing this
            let exePath = ms.profTBExePath.Text.Trim()
            let exeExists = File.Exists exePath

            let hasProfiles = ms.lbProfiles.Items.Count > 0

            if hasProfiles && (not exeExists) then
                errDialog("Current profile has an invalid executable path; please fix or delete the profile")
            else
                let newLbl = ms.lbProfiles.Items.Add({ EmptyProfile with DisplayName = pendingProfileName}) 
                ms.lbProfiles.SelectedIndex <- newLbl
                ms.profTBExePath.Text <- ""
                ms.probTBModsPath.Text <- "") //TODO build from global default and exe path

        ms.profBtnExeBrowse.Click.Add(fun (evArgs) ->
            let checkForExistingProfile exePath = RegConfig.findProfile exePath
            let createProfile exePath = RegConfig.saveProfile({CoreTypes.DefaultRunConfig with ExePath = exePath})

            match getSelectedProfile() with
            | :? UIProfile as uiProfile -> 
                // TODO: bah, naming here sucks - need better distinction between the registy profile
                // and the ui profile stored in the list box.
                let oldPathValid = File.Exists(ms.profTBExePath.Text)

                let resExe = selectExecutableDialog(ms)
                match resExe,oldPathValid with
                | Some exePath,_ when File.Exists(exePath) -> 
                    // make sure no other profile exists with same path
                    match checkForExistingProfile exePath with
                    | Some profile -> errDialog("A profile already exists for this executable")
                    | None -> 
                        // create a new profile and save
                        createProfile exePath
                        updateUiProfile(uiProfile,exePath)
                        // TODO: this shouldn't be a textbox; must always be a valid exe, therefore only settable via
                        // browse button
                        ms.profTBExePath.Text <- exePath 
                | _,true ->
                    // no new selection, but old selection still valid, so don't change it
                    () 
                | _,false -> 
                    updateUiProfile(uiProfile,"")
                    ms.profTBExePath.Text <- ""
            | _ -> errDialog("Please select a profile or create a new one"))

        let updateSelectedProfile() = 
            // TODO: btn create mod should be here too; do this after I've added the profile logic
            // TODO: ms.profBtnExeBrowse should only be enabled if at least one profile is present and selected
            let requireActive = [ms.btnStartPlayback; ms.btnStartSnap; ms.btnDeleteProfile; ]

            match ms.lbProfiles.SelectedItem with 
            | null -> 
                requireActive |> List.iter (fun ctrl -> ctrl.Enabled <- false)
            | profile -> 
                requireActive |> List.iter (fun ctrl -> ctrl.Enabled <- true)
            ()

        updateSelectedProfile()

        updateState { state.Value with MainScreenForm = ms }
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
