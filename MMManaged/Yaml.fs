// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

namespace ModelMod

open System
open System.IO

open YamlDotNet.RepresentationModel

module Yaml =
    let load (filename:string) = 
        use input = new StringReader(File.ReadAllText(filename))
        let yamlStream = new YamlStream()
        yamlStream.Load(input)
        yamlStream.Documents

    let toString (node:YamlNode) =
        match node with 
        | :? YamlScalarNode as scalar -> 
            scalar.Value
        | _ -> failwithf "Cannot extract string from node %A" node

    let toOptionalString (node:YamlNode option) =
        match node with 
        | None -> None
        | Some n -> Some (toString(n))

    let toInt (node:YamlNode) =
        match node with 
        | :? YamlScalarNode as scalar -> 
            Convert.ToInt32 scalar.Value
        | _ -> failwithf "Cannot extract string from node %A" node
        
    let toBool (defval:bool) (node:YamlNode option) =
        match node with
        | None -> defval
        | Some x -> Convert.ToBoolean(toString(x))

    let getOptionalValue (key:string) (mapNode:YamlMappingNode) = 
        let key = key.ToLowerInvariant()

        let nValue = mapNode.Children |> Seq.tryFind (fun (pair) -> pair.Key.ToString().ToLower() = key ) 
        match nValue with 
            | None -> None
            | Some(s) -> Some (s.Value)

    let getValue (key:string) (mapNode:YamlMappingNode) = 
        let key = key.ToLower()
        let nValue = getOptionalValue key mapNode
        match nValue with 
            | None -> failwithf "Required value '%s' not found in node type '%A'" key mapNode
            | Some v -> v
    
    let toOptionalSequence (node:YamlNode option) =
        match node with
        | None -> None
        | Some thing ->
            match thing with
            | :? YamlSequenceNode as ySeq -> Some ySeq
            | _ -> failwithf "Expected sequence type, but got %A" thing

    let toSequence (failMsg:string) (node:YamlNode) =
        let s = toOptionalSequence(Some(node))
        match s with
        | None -> failwith failMsg
        | Some s -> s

    let toOptionalMapping (node:YamlNode option) =
        match node with
        | None -> None
        | Some thing -> 
            match thing with 
            | :? YamlMappingNode -> 
                let yml = thing :?> YamlMappingNode
                Some yml
            | _ -> failwithf "Expected mapping node type, but got %A" thing

    let toMapping (failMsg:string) (node:YamlNode) =
        let mapping = toOptionalMapping(Some(node))
        match mapping with
        | None -> failwith failMsg
        | Some m -> m
