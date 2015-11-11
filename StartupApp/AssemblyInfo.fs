namespace System
open System.Reflection
open System.Runtime.InteropServices

[<assembly: AssemblyTitleAttribute("ModelMod launcher 'shortcut'")>]
[<assembly: AssemblyDescriptionAttribute("")>]
[<assembly: GuidAttribute("df438f0d-1e48-42d2-bc4d-7b3500c48515")>]
[<assembly: AssemblyProductAttribute("ModelMod")>]
[<assembly: AssemblyVersionAttribute("1.0.0.6")>]
[<assembly: AssemblyFileVersionAttribute("1.0.0.6")>]
do ()

module internal AssemblyVersionInformation =
    let [<Literal>] Version = "1.0.0.6"
