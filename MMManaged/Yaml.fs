// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU Lesser General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

namespace ModelMod

open System
open System.IO

open YamlDotNet.RepresentationModel

/// Helper functions for working with YamlDotNet in an fsharpy way.
module Yaml =
    /// Load a yaml file and return the root documents collection.
    let load (filename:string) = 
        use input = new StringReader(File.ReadAllText(filename))
        let yamlStream = new YamlStream()
        yamlStream.Load(input)
        yamlStream.Documents

    /// Convert a node to a string.  Throws exception on failure.
    let toString (node:YamlNode) =
        match node with 
        | :? YamlScalarNode as scalar -> 
            scalar.Value
        | _ -> failwithf "Cannot extract string from node %A" node

    /// Convert the node to Some(toString(node)), or returns None if node is None.
    /// Throws exception if the node is Some but the conversion fails.
    let toOptionalString (node:YamlNode option) =
        node |> Option.map (fun n -> toString(n))

    let toOptionalBool (node: YamlNode option) = 
        node |> Option.map (fun n -> 
            match n with 
            | :? YamlScalarNode as scalar -> 
                Convert.ToBoolean scalar.Value
            | _ -> failwithf "Cannot extract bool from node %A" node
        )

    /// Convert the node to an integer.  Throws exception on failure.
    let toInt (node:YamlNode) =
        match node with 
        | :? YamlScalarNode as scalar -> 
            Convert.ToInt32 scalar.Value
        | _ -> failwithf "Cannot extract int from node %A" node

    let toOptionalInt (node:YamlNode option) =
        node |> Option.map (fun n -> toInt(n))
        
    /// Convert the node to a boolean.  If node is None, returns the default value.  If some,
    /// returns the converted boolean, or throws exception if it cannot be converted.
    let toBool (defval:bool) (node:YamlNode option) =
        match node with
        | None -> defval
        | Some x -> Convert.ToBoolean(toString(x))

    /// Returns a value from the mapping, or None if the key is not found.
    let getOptionalValue (key:string) (mapNode:YamlMappingNode) = 
        let key = key.ToLowerInvariant()

        let nValue = mapNode.Children |> Seq.tryFind (fun (pair) -> pair.Key.ToString().ToLower() = key ) 
        match nValue with 
        | None -> None
        | Some(s) -> Some (s.Value)

    /// Returns a value form the mapping, or throws exception if the key is not found.
    let getValue (key:string) (mapNode:YamlMappingNode) = 
        let key = key.ToLower()
        let nValue = getOptionalValue key mapNode
        match nValue with 
        | None -> failwithf "Required value '%s' not found in node type '%A'" key mapNode
        | Some v -> v

    /// Walks the list of keys and returns the first value found in the mapping.
    /// Throws exception if none found.
    let getFirstValue (keys:string list) (mapNode:YamlMappingNode) = 
        let found = keys |> List.tryPick (fun key -> getOptionalValue key mapNode)
        match found with
        | None -> failwithf "No value found for any key in '%A' in node type '%A'" keys mapNode
        | Some v -> v
    
    /// Convert the node to Some(YamlSequenceNode)), or returns None if node is None.
    /// Throws exception if the node is Some but the conversion fails.
    let toOptionalSequence (node:YamlNode option) =
        node |> Option.map (fun thing -> 
            match thing with
            | :? YamlSequenceNode as ySeq -> ySeq
            | _ -> failwithf "Expected sequence type, but got %A" thing)

    /// Convert the node to a YamlSequenceNode.  If conversion fails, throw exception with the specified fail message.
    let toSequence (failMsg:string) (node:YamlNode) =
        let s = toOptionalSequence(Some(node))
        match s with
        | None -> failwith failMsg
        | Some s -> s

    /// Convert the node to Some(YamlMappingNode)), or returns None if node is None.
    /// Throws exception if the node is Some but the conversion fails.
    let toOptionalMapping (node:YamlNode option) =
        match node with
        | None -> None
        | Some thing -> 
            match thing with 
            | :? YamlMappingNode -> 
                let yml = thing :?> YamlMappingNode
                Some yml
            | _ -> failwithf "Expected mapping node type, but got %A" thing

    /// Convert the node to a mapping node.  Throws exception if the conversion fails.
    let toMapping (failMsg:string) (node:YamlNode) =
        let mapping = toOptionalMapping(Some(node))
        match mapping with
        | None -> failwith failMsg
        | Some m -> m