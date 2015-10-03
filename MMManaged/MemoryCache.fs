namespace ModelMod

open System.IO

open CoreTypes
open InteropTypes

open System.Collections.Generic

// This is a simple in-memory cache for loaded meshes.  It speeds up reload iteration time,
// since only modified meshes are reloaded.  Would be nice to extend this to other things
// (yaml, meshrelation), since time spent in those can add up as well.
// To force a clear of the cache, use the full reload keybinding to reload the 
// whole assembly.
module MemoryCache =
    let private log = Logging.getLogger("MemoryCache")

    type CacheEntry = {
        Type:ModType
        Flags:MeshReadFlags
        Mesh:Mesh
        MTime:System.DateTime
    }

    let cache = new Dictionary<string,CacheEntry>()

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
