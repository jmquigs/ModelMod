﻿namespace System
open System.Reflection
open System.Runtime.InteropServices

[<assembly: AssemblyTitleAttribute("ModelMod managed code library")>]
[<assembly: AssemblyDescriptionAttribute("")>]
[<assembly: GuidAttribute("13c62567-ab30-4954-9c47-213bc2a0ab7e")>]
[<assembly: AssemblyProductAttribute("ModelMod")>]
[<assembly: AssemblyVersionAttribute("1.2.0.0")>]
[<assembly: AssemblyFileVersionAttribute("1.2.0.0")>]
do ()

module internal AssemblyVersionInformation =
    let [<Literal>] Version = "1.2.0.0"
