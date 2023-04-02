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

open System.IO

open CoreTypes
open InteropTypes

open System.Collections.Generic

/// This is a simple in-memory cache for loaded meshes.  It speeds up reload iteration time,
/// since only modified meshes are reloaded.  Would be nice to extend this to other things
/// (yaml, meshrelation), since time spent in those can add up as well.
/// To force a clear of the cache, use the full reload keybinding to reload the
/// whole assembly.
module MemoryCache =
    let private log = Logging.getLogger("MemoryCache")

    type CacheEntry = {
        Type:ModType
        Flags:MeshReadFlags
        Mesh:Mesh
        MTime:System.DateTime
    }

    let cache = new Dictionary<string,CacheEntry>()

    let clear() = cache.Clear()

    let get (path, (modType:ModType), flags):Mesh option =
        let ok,entry = cache.TryGetValue path
        if ok then
            if entry.Type = modType && entry.Flags = flags then
                let mtime = File.GetLastWriteTime(path)

                if mtime <= entry.MTime then
                    log.Info "Cache hit for file: %s" path
                    Some(entry.Mesh)
                else
                    log.Info "File updated, reloading: %s" path
                    None
            else
                log.Warn "Unusable asset cache file; flags or mod type mismatch: %s" path
                None
        else
            log.Info "Cache miss for file: %s" path
            None

    let save(path, (modType:ModType), flags, (mesh:Mesh)) =
        let mtime = File.GetLastWriteTime(path)
        let entry = {
            Type = modType
            Flags = flags
            Mesh = mesh
            MTime = mtime
        }

        cache.[path] <- entry
