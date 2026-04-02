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

/// Interface that the hot-reloadable engine assembly must implement.
/// The shell (MMManaged.dll) loads the engine assembly dynamically and delegates
/// all callbacks through this interface. On hot-reload, a new engine assembly is
/// loaded and a fresh IEngineModule instance replaces the previous one.
type IEngineModule =
    /// Initialize the engine module with the logging factory and native context string.
    /// Called once after the engine assembly is loaded (or reloaded).
    abstract Initialize: logFactory:Logging.LoggerFactory -> context:string -> unit

    /// Return a ManagedCallbacks struct populated with delegates pointing to the
    /// engine implementation's functions. The shell will forward native calls through
    /// these delegates.
    abstract GetCallbacks: unit -> MMNative.ManagedCallbacks
