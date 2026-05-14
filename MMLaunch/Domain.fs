// Slim domain types for the launcher. The full versions live in MMManaged but pull
// in MonoGame/SharpDX (.NET Framework only); the launcher only needs the registry-
// backed RunConfig/GameProfile records and a class shape compatible with
// SnapshotProfile descriptions.

namespace ModelMod

open System

// The actual Domain contents from MMManaged are included as files (see MMLaunch.fsproj file)