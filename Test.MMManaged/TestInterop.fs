module TestInterop

open NUnit.Framework

open ModelMod

/// Main.ParseNativeModule is pure, so these cases can be checked without native code.  The
/// argument layout mirrors what reload_managed_dll in the dnclr crate builds: handle, context,
/// version, then named entries.
let private baseArgs = [| "12345"; "d3d11"; "6" |]

[<Test>]
let ``ParseNativeModule: absent argument falls back to context``() =
    Assert.AreEqual("d3d11", Main.ParseNativeModule(baseArgs, "d3d11"))
    Assert.AreEqual("d3d9", Main.ParseNativeModule([| "1"; "d3d9"; "6"; "mod_structsize=10" |], "d3d9"))

[<Test>]
let ``ParseNativeModule: present argument is returned``() =
    let args = Array.append baseArgs [| "native_module=d3d11_mm" |]
    Assert.AreEqual("d3d11_mm", Main.ParseNativeModule(args, "d3d11"))
    let args = Array.append baseArgs [| "native_module=d3d11" |]
    Assert.AreEqual("d3d11", Main.ParseNativeModule(args, "d3d11"))

[<Test>]
let ``ParseNativeModule: key and value are matched case-insensitively and lowercased``() =
    let args = Array.append baseArgs [| "Native_Module=D3D11_MM" |]
    Assert.AreEqual("d3d11_mm", Main.ParseNativeModule(args, "d3d11"))

[<Test>]
let ``ParseNativeModule: empty value falls back to context``() =
    let args = Array.append baseArgs [| "native_module=" |]
    Assert.AreEqual("d3d11", Main.ParseNativeModule(args, "d3d11"))
    let args = Array.append baseArgs [| "native_module=   " |]
    Assert.AreEqual("d3d11", Main.ParseNativeModule(args, "d3d11"))

[<Test>]
let ``ParseNativeModule: found among the struct size entries``() =
    let args = Array.append baseArgs [| "mod_structsize=100"; "native_module=d3d11_mm"; "mod_snapprofile_structsize=200" |]
    Assert.AreEqual("d3d11_mm", Main.ParseNativeModule(args, "d3d11"))
    let args = Array.append baseArgs [| "mod_structsize=100"; "mod_snapprofile_structsize=200"; " native_module=d3d11_mm " |]
    Assert.AreEqual("d3d11_mm", Main.ParseNativeModule(args, "d3d11"))

[<Test>]
let ``ParseNativeModule: entries before index 3 are never treated as the module``() =
    // the context slot itself must not be mistaken for the argument
    Assert.AreEqual("d3d11", Main.ParseNativeModule([| "native_module=x"; "native_module=y"; "native_module=z" |], "d3d11"))
    Assert.AreEqual("d3d11", Main.ParseNativeModule([||], "d3d11"))
