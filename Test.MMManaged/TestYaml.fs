module TestYaml

open FsUnit
open FsCheck
open NUnit.Framework
open System.IO
open System.Reflection

open ModelMod

let loadTestDoc() =
    let ydocs = Yaml.load (Path.Combine(Util.TestDataDir, "Test.yaml"))
    Check.QuickThrowOnFailure (ydocs.Count = 1 |@ sprintf "incorrect number of yaml documents: %A" ydocs)
    let doc = ydocs.Item(0)
    let mapNode = Yaml.toMapping "root is not a mapping node" doc.RootNode
    mapNode

[<Test>]
let ``Yaml: module functions``() =
    let checkFails x = Check.QuickThrowOnFailure <| Prop.throws<System.Exception,_> (x)
    let check x = Check.QuickThrowOnFailure x

    let mapNode = loadTestDoc()

    // toString
    let s = mapNode |> Yaml.getValue "Something" |> Yaml.toString
    check (s = "Somewhere" |@ sprintf "incorrect string: %A" s)
    checkFails (lazy (mapNode |> Yaml.getValue "Mapping" |> Yaml.toString))

    // toOptionalString
    let s = mapNode |> Yaml.getOptionalValue "Something" |> Yaml.toOptionalString
    check (Option.get s = "Somewhere" |@ sprintf "incorrect string: %A" s)
    let s = mapNode |> Yaml.getOptionalValue "Missing" |> Yaml.toOptionalString
    check (s = None |@ sprintf "incorrect string: %A" s)

    // toInt
    let s = mapNode |> Yaml.getValue "Int" |> Yaml.toInt
    check (s = 47 |@ sprintf "incorrect int: %A" s)
    checkFails (lazy (mapNode |> Yaml.getValue "Something" |> Yaml.toInt))

    // toBool
    let s = mapNode |> Yaml.getOptionalValue "bool" |> Yaml.toBool false
    check (s = true |@ sprintf "incorrect bool: %A" s)
    let s = mapNode |> Yaml.getOptionalValue "missing" |> Yaml.toBool true
    check (s = true |@ sprintf "incorrect bool: %A" s)

    // sequence
    let s = mapNode |> Yaml.getValue "Sequence" |> Yaml.toSequence "expected a sequence" |> Seq.map Yaml.toInt |> List.ofSeq
    check (s = [1;2;3;4;5] |@ sprintf "incorrect sequence: %A" s)
    checkFails (lazy (mapNode |> Yaml.getValue "Mapping" |> Yaml.toSequence "expected a sequence"))

    // mapping
    let _ =
        let mapNode = mapNode |> Yaml.getValue "Mapping" |> Yaml.toMapping "a mapping is required"
        
        let s = mapNode |> Yaml.toMapping  "Mapping"
        let aval = s |> Yaml.getValue "a" |> Yaml.toInt
        check (aval = 1 |@ sprintf "incorrect value for a: %A" aval)
        let bval = s |> Yaml.getValue "b" |> Yaml.toInt
        check (bval = 2 |@ sprintf "incorrect value for b: %A" bval)

        checkFails (lazy (mapNode |> Yaml.getValue "Something" |> Yaml.toMapping "a mapping is required"))
    ()
        
        

    

