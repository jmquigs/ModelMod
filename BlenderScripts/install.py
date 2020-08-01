# this script is executed by mmlaunch to enable the mmobj plugin

import addon_utils
import bpy
import sys

# note, the UI checks for this string because we can't return error codes (see below)
SUCCESS="MMINFO: Plugin enabled"

outHandle = None

def wline(line):
    if outHandle != None:
        print(line, end="\n", file=outHandle)
    else:
        print(line)

def install_mmobj():
    enabled,_ = addon_utils.check('io_scene_mmobj')
    if not enabled:
        try:
            # this interface changed in 2.77+, try old way first
            addon_utils.enable('io_scene_mmobj', True)
        except TypeError:
            addon_utils.enable('io_scene_mmobj', default_set=True, persistent=True)
        enabled,_ = addon_utils.check('io_scene_mmobj')
        if enabled:
            bpy.ops.wm.save_userpref()
            wline(SUCCESS) 
        else:
            # sys.exit(1) # this causes blender to die messily without setting the return code.  so we'll just have to print the error
            wline("MMERROR: Plugin failed to enable; install path may not be correct, or it is incompatible with this version of blender")
    else:
        wline(SUCCESS)

def show_paths():
    for p in addon_utils.paths():
        wline("MMPATH:" + p)
    
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
    argc = len(args)
    if argc == 0:
        command = defaultCommand
    elif argc == 1:
        command = args[0]
    elif argc >= 2:
        command = args[0]
        outHandle = open(args[1], "w")
        
if not command in commands:
    print("unknown command")
else:
    commands[command]()
if outHandle != None:
    outHandle.close()