namespace ModelMod

open System

open YamlDotNet.RepresentationModel

module Yaml =
    let getScalar (node:YamlNode) = 
        match node with 
        | :? YamlScalarNode as nodeType -> 
            Some (nodeType.Value)
        | _ -> None

    let getRequiredString (node:YamlNode) =
        match node with 
        | :? YamlScalarNode as scalar -> 
            scalar.Value
        | _ -> failwithf "Cannot extract string from node %A" node; ""

    let getRequiredInt (node:YamlNode) =
        match node with 
        | :? YamlScalarNode as scalar -> 
            Convert.ToInt32 scalar.Value
        | _ -> failwithf "Cannot extract string from node %A" node
        
    let getString (node:YamlNode option) =
        match node with 
        | None -> failwithf "Cannot extract string from empty node"
        | Some n -> 
            getRequiredString(n)

    let getOptionalString (node:YamlNode option) =
        match node with 
        | None -> None
        | Some n -> Some (getRequiredString(n))
        
    let getOptionalBool (defval:bool) (node:YamlNode option) =
        match node with
        | None -> defval
        | Some x -> Convert.ToBoolean(getRequiredString(x))

    let getOptionalValue (mapNode:YamlMappingNode) (key:string) = 
        let key = key.ToLowerInvariant()

        let nValue = mapNode.Children |> Seq.tryFind (fun (pair) -> pair.Key.ToString().ToLower() = key ) 
        match nValue with 
            | None -> None
            | Some(s) -> Some (s.Value)

    let getRequiredValue (mapNode:YamlMappingNode) (key:string) = 
        let key = key.ToLower()
        let nValue = getOptionalValue mapNode key
        match nValue with 
            | None -> failwithf "Required value '%s' not found in node type '%A'" key mapNode
            | _ -> ()
        nValue
    
    let getSequence (node:YamlNode option) =
        match node with
        | None -> None
        | Some thing ->
            match thing with
            | :? YamlSequenceNode as ySeq -> Some ySeq
            | _ -> failwithf "Expected sequence type, but got %A" thing

    let getRequiredSequence failMsg (node:YamlNode option) =
        let s = getSequence(node)
        match s with
        | None -> failwith failMsg
        | Some s -> s

    let getMapping (node:YamlNode option) =
        match node with
        | None -> None
        | Some thing -> 
            match thing with 
            | :? YamlMappingNode -> 
                let yml = thing :?> YamlMappingNode
                Some yml
            | _ -> failwithf "Expected mapping node type, but got %A" thing

    let getRequiredMapping (failMsg: string) (node:YamlNode option) =
        let mapping = getMapping(node)
        match mapping with
        | None -> failwith failMsg
        | Some m -> m
