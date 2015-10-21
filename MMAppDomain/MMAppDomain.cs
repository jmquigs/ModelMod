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

using System;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;

namespace MMAppDomain
{
    // If this interface is changed, rebuild the .tlb with the following command so that MMInterop in ModelMod can see the change.  
    // (From visual studio prompt):
    // tlbexp bin\Debug\ModelModCLRAppDomain.dll /out:MMAppDomain.tlb
    [ComVisible(true)]
    [GuidAttribute("877C89BF-96B0-4279-9B88-9440EFA33AB3")]
    public interface IMMAppDomainMananger
    {
        String Load(String assembly);
    }

    // This is the root app domain manager.  It is implemented in C# so that we don't have a dependency on 
    // FSharp.Core just to start the CLR, since its not easy to set the app base path for this module to a 
    // location that contains a private copy of FSharp.Core (or perhaps it easy, but I don't know how to do so.)
    // This module also creates an internal app domain
    // for actually running the modelmod code; we can set the app base easily for this domain.  So we do that,
    // and thus avoid requiring FSharp.Core and other libraries to be installed in the GAC.  Using an internal
    // App Domain also allows us to hot swap the F# code.
    [ComVisible(true)]
    [GuidAttribute("05A26D5C-430A-4351-9FF1-52762B680716")]
    public class MMAppDomainManager : AppDomainManager, IMMAppDomainMananger
    {
        static MMAppDomainManager()
        {
            WriteLog("Static Init Called");
        }
        private AppDomainSetup _appDomainInfo;
        private AppDomain _currentDomain;

        public static void WriteLog(String msg)
        {
            var logFile = @"C:\Dev\Temp\AppDomainLog.txt";
            File.AppendAllText(logFile, msg + @"\r\n");
        }

        public override void InitializeNewDomain(AppDomainSetup appDomainInfo) 
        {
            WriteLog("InitNewDomain");
            WriteLog(appDomainInfo.ToString());
            base.InitializeNewDomain(appDomainInfo);
            base.InitializationFlags = AppDomainManagerInitializationOptions.RegisterWithHost;
            _appDomainInfo = appDomainInfo;
        }

        // This callback is used to load the main assembly.  This must be static, otherwise it will silently
        // run in the main appdomain, not the specific one that we create for modelmod use; this will most 
        // likely then fail because it can't find FSharp.Core (unless it happens to be installed on the system).
        private static void LoaderCB()
        {
            WriteLog("LoaderCB");

            var myBase = AppDomain.CurrentDomain.BaseDirectory;
            var myDomain = AppDomain.CurrentDomain;

            try
            {
                var assembly = AppDomain.CurrentDomain.SetupInformation.ConfigurationFile;
                var raw = File.ReadAllBytes(assembly);
                var asm = Assembly.Load(raw);
                var MainType = "ModelMod.Main";
                var mmMainType = asm.GetType(MainType);
                if (mmMainType == null)
                {
                    throw new Exception("Unable to locate main type " + MainType + " in assembly " + assembly);
                }
                var flags = BindingFlags.Public | BindingFlags.Static;
                var staticMain = mmMainType.GetMethod("Main", flags);
                if (staticMain == null)
                {
                    throw new Exception("Unable to locate static main method 'Main' in main type: " + MainType);
                }
                Object[] args = { "dummy argument" };

                // This Invoke can fail, but in some cases, 
                // the exception doesn't seem to be thrown on the native thread.
                // Missing assembly is one of those cases.  
                // Another case where the callback can fail is if a native entry point (referenced by Interop.fs)
                // cannot be found.
                WriteLog("Invoking SM");
                staticMain.Invoke(null, args);
                WriteLog("Done");
            }

            catch (Exception e)
            {
                StringWriter sw = new StringWriter();
                sw.WriteLine("Error during sandbox init.");
                sw.WriteLine("Sandbox App Base: " + myBase);
                sw.WriteLine("Sandbox App Domain: " + myDomain.GetHashCode());

                // this should end up in modelmod.log
                throw new Exception(sw.ToString(), e);
            }
        }

        public String Load(String assembly)
        {
            WriteLog("Load asm: " + assembly);

            String domainBase = "unknown";
            try
            {
                if (_currentDomain != null)
                {
                    AppDomain.Unload(_currentDomain);
                }

                _currentDomain = null;

                // The assembly is expected to be in the ModelMod installation directory.
                // (Unlike this app domain assembly, which must be copied into the game dir).
                // Setting appbase to this modelmod install dir allows the references 
                // (fsharp, monogame, etc) to be loaded 
                // without requiring them to be copied to the game dir.

                var ads = new AppDomainSetup();
                ads.ApplicationBase = Path.GetDirectoryName(assembly);
                ads.DisallowBindingRedirects = false;
                ads.DisallowCodeDownload = true;
                // use the configuration file property to specify the name of the assembly that must be loaded.
                // can't use a static field; the sandbox domain has a separate copy of all the statics.
                ads.ConfigurationFile = assembly;

                // this loader callback is needed so that we can hot-swap the assembly.
                // http://stackoverflow.com/questions/425077/how-to-delete-the-pluginassembly-after-appdomain-unloaddomain
                var d = base.CreateDomain("ImSoFriendly", null, ads);
                _currentDomain = d;
                domainBase = d.BaseDirectory; 
                d.DoCallBack(new CrossAppDomainDelegate(LoaderCB));
                
                return ""; // empty string means no error, otherwise, its an error message
            }
            catch (Exception e)
            {
                return "Error during load. " + Environment.NewLine 
                    + "Root app domain: " + AppDomain.CurrentDomain.GetHashCode() + Environment.NewLine 
                    + "Sandbox domain: " + _currentDomain.GetHashCode() + Environment.NewLine
                    + "Sandbox domain base: " + domainBase + Environment.NewLine
                    + "Exception: " + e.ToString();
            }
        }
    }
}
