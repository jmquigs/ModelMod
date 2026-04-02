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

namespace ModelMod.Engine

open ModelMod

/// Entry point for the hot-reloadable engine assembly.
/// Implements IEngineModule which the shell uses to obtain callback delegates.
type EngineEntry() =
    interface IEngineModule with
        member _.Initialize (logFactory: Logging.LoggerFactory) (context: string) =
            // Set up logging using the factory provided by the shell.
            // This routes all log messages from the engine assembly through the
            // same native logging infrastructure.
            Logging.setLoggerFactory logFactory

            // Set the runtime context so that engine code knows which D3D version is in use.
            CoreState.Context <- context

        member _.GetCallbacks () =
            {
                MMNative.ManagedCallbacks.SetPaths = new MMNative.SetPathsCB(ModDBInterop.setPaths)
                LoadModDB = new MMNative.LoadModDBCB(ModDBInterop.loadFromDataPathAsync)
                GetModCount = new InteropTypes.GetModCountCB(ModDBInterop.getModCount)
                GetModData = new InteropTypes.GetModDataCB(ModDBInterop.getModData)
                FillModData = new InteropTypes.FillModDataCB(ModDBInterop.fillModData)
                LoadModData = new InteropTypes.LoadModDataCB(ModDBInterop.loadModData)
                TakeSnapshot = new InteropTypes.TakeSnapshotCB(Snapshot.take)
                GetLoadingState = new InteropTypes.GetLoadingStateCB(ModDBInterop.getLoadingState)
                GetSnapshotResult = new InteropTypes.GetSnapshotResultCB(Snapshot.getResult)
            }
