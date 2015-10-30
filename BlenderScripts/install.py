# this script is executed by mmlaunch to enable the mmobj plugin

import addon_utils
import bpy
import sys

# note, the UI checks for this string because we can't return error codes (see below)
SUCCESS="MMINFO: Plugin enabled"

def install_mmobj():
    enabled,_ = addon_utils.check('io_scene_mmobj')
    if not enabled:
        addon_utils.enable('io_scene_mmobj', True)
        enabled,_ = addon_utils.check('io_scene_mmobj')
        if enabled:
            bpy.ops.wm.save_userpref()
            print(SUCCESS) 
        else:
            # sys.exit(1) # this causes blender to die messily without setting the return code.  so we'll just have to print the error
            print("MMERROR: Plugin failed to enable; install path may not be correct, or it is incompatible with this version of blender")
    else:
        print(SUCCESS)

def show_paths():
    for p in addon_utils.paths():
        print("MMPATH:",p)
    
defaultCommand = "paths"
command = ""

commands = {
    "paths": show_paths,
    "install": install_mmobj
}

ddIdx = sys.argv.index('--') if '--' in sys.argv else None
if ddIdx == None:
    command = defaultCommand
else:
    args = sys.argv[(ddIdx+1):]
    if len(args) == 0:
        command = defaultCommand
    else:
        command = args[0]
        
if not command in commands:
    print("unknown command")
else:
    commands[command]()
