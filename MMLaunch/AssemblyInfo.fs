namespace System
open System.Reflection
open System.Runtime.InteropServices

[<assembly: AssemblyTitleAttribute("ModelMod launcher application")>]
[<assembly: AssemblyDescriptionAttribute("")>]
[<assembly: GuidAttribute("2ce8e338-7143-4f97-ab39-3e90ca50bdf2")>]
[<assembly: AssemblyProductAttribute("ModelMod")>]
[<assembly: AssemblyVersionAttribute("1.1.0.0")>]
[<assembly: AssemblyFileVersionAttribute("1.1.0.0")>]
do ()

module internal AssemblyVersionInformation =
    let [<Literal>] Version = "1.1.0.0"
