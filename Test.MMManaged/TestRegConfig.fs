module TestRegConfig

open System

open NUnit.Framework

open ModelMod

/// pickBestProfile is registry-free, so these cases can be checked directly.  They mirror the
/// tests on util::game_profile::pick_best_profile in the native code; keep the two in sync.
let private matched exePath profiles =
    match RegConfig.pickBestProfile exePath profiles with
    | Some key -> key
    | None -> "<none>"

[<Test>]
let ``RegConfig: path components ignore separator style``() =
    Assert.AreEqual("c:|games|foo|game.exe",
        String.Join("|", RegUtil.pathComponents @"c:/games\foo//game.exe"))
    Assert.AreEqual("?|s:|foo|game.exe",
        String.Join("|", RegUtil.pathComponents @"\\?\s:\foo\game.exe"))

[<Test>]
let ``RegConfig: common suffix length``() =
    let a = RegUtil.pathComponents @"s:\steamapps\common\foo\bin\game.exe"
    let b = RegUtil.pathComponents @"z:\home\me\steamapps\common\foo\bin\game.exe"
    Assert.AreEqual(5, RegUtil.commonSuffixLen a b)
    Assert.AreEqual(0, RegUtil.commonSuffixLen a (RegUtil.pathComponents @"c:\other.exe"))

[<Test>]
let ``RegConfig: exact profile match ignores case and whitespace``() =
    let p = [| "P0", "  C:\\Games\\Foo\\Bin\\Game.exe  " |]
    Assert.AreEqual("P0", matched @"c:\games\foo\bin\game.exe" p)

[<Test>]
let ``RegConfig: profile matches same install through different drive mappings``() =
    // the profile was created against the Z: (unix root) view; the game is launched through a
    // steam library drive.
    let p = [| "P0", @"Z:\home\me\SteamLibrary\steamapps\common\Foo\bin\game.exe" |]
    Assert.AreEqual("P0", matched @"S:\steamapps\common\Foo\bin\game.exe" p)
    // a hand-made mapping straight at the game dir still shares 3 components.
    Assert.AreEqual("P0", matched @"C:\Foo\bin\game.exe" p)

[<Test>]
let ``RegConfig: profile match ignores separator style and object dir prefix``() =
    let p = [| "P0", "S:/steamapps/common/Foo/bin/game.exe" |]
    Assert.AreEqual("P0", matched @"\\?\S:\steamapps\common\Foo\bin\game.exe" p)

[<Test>]
let ``RegConfig: profile file name alone is not enough``() =
    let p = [| "P0", @"C:\Games\Bar\game.exe" |]
    Assert.AreEqual("<none>", matched @"C:\Games\Foo\game.exe" p)

[<Test>]
let ``RegConfig: best profile score wins over a weaker match``() =
    let p = [|
        "P0", @"C:\Games\Bar\bin\game.exe"
        "P1", @"Z:\home\me\steamapps\common\Foo\bin\game.exe"
    |]
    Assert.AreEqual("P1", matched @"S:\steamapps\common\Foo\bin\game.exe" p)

[<Test>]
let ``RegConfig: empty profile paths never match``() =
    Assert.AreEqual("<none>", matched @"C:\Games\Foo\game.exe" [| "P0", ""; "P1", "   " |])
    Assert.AreEqual("<none>", matched "" [| "P0", @"C:\Games\Foo\game.exe" |])

[<Test>]
let ``RegConfig: profile ties resolve to the first in registry order``() =
    let p = [|
        "P0", @"S:\steamapps\common\Foo\bin\game.exe"
        "P1", @"Z:\home\me\steamapps\common\Foo\bin\game.exe"
    |]
    // both share "common\Foo\bin\game.exe"; P0 wins on order, not on score.
    Assert.AreEqual("P0", matched @"C:\steamapps\common\Foo\bin\game.exe" p)
