#pragma once

#define DIRECTINPUT_VERSION 0x0800
#include <dinput.h>

#include "Types.h"

#include <vector>
using namespace std;

typedef HRESULT (_stdcall *DInputProc)(HINSTANCE,DWORD,REFIID,LPVOID,LPUNKNOWN);

namespace ModelMod {	
class Input
{
IDirectInput8* _dinput;
LPDIRECTINPUTDEVICE8  _lpdiKeyboard; 
char _keyboardState[256];
char _lastKeyboardState[256];
Uint16 _repeatDelay[256];

DWORD _lastUpdate;
DWORD _lastPressEvent[256];
bool _altPressed;
bool _shiftPressed;
bool _ctrlPressed;

public:
	Input(void);
	~Input(void);

	struct KeyEvent {
		Uint8 key;
		bool pressed;

		KeyEvent() { 
			key = 255;
			pressed = false;
		}

		KeyEvent(Uint8 key, bool pressed) {
			this->key = key;
			this->pressed = pressed;
		}
	};

	bool init();

	bool isInitialized(); 

	vector<KeyEvent> update();
	void reset();

	bool isKeyPressed(int key) { return (_keyboardState[key] & 0x80) > 0; }
	bool isAltPressed() { return _altPressed; }
	bool isCtrlPressed() { return _ctrlPressed; }
	bool isShiftPressed() { return _shiftPressed; }
};

};