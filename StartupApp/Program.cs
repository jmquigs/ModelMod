using System;
using System.IO;
using System.Diagnostics;

namespace StartupApp
{
    static class Program
    {
        // The sole purpose of this app is to have a "ModelMod.exe" file that we can stick 
        // in the root folder so that the user doesn't have to go into "Bin" and randomly
        // click executables there.  Obviously an installer would be another way to 
        // handle this, but installers are generally yucky (especially ones that 
        // require effing elevated privileges) and I'm too lazy to make one.
        // This is written in CS so that we don't need an Fsharp.Core outside the bin folder.
        [STAThread]
        static void Main()
        {
            String[] paths = { @".", @".\Bin" };
            String target = "MMLaunch.exe";

            String found = null;

            foreach (String p in paths) {
                var path = Path.Combine(p, target);
                if (File.Exists(path))
                {
                    found = path;
                    break;
                }
            }

            if (found != null)
            {
                var proc = new Process();
                proc.StartInfo.UseShellExecute = false;
                proc.StartInfo.FileName = found;
                proc.StartInfo.WorkingDirectory = Path.GetDirectoryName(found);
                proc.Start();
            }
        }
    }
}
