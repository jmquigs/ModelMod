# this script is executed by mmlaunch to enable the mmobj plugin

import addon_utils
import bpy
import sys
enabled,_ = addon_utils.check('io_scene_mmobj')
if not enabled:
	addon_utils.enable('io_scene_mmobj')
	enabled,_ = addon_utils.check('io_scene_mmobj')
	#if not enabled:
	#	sys.exit(1) # todo: this causes blender to die messily without setting this return code.

	bpy.ops.wm.save_userpref()
