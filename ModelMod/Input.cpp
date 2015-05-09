#include "Input.h"
#include "Log.h"

static const string LogCategory = "Input";

// these are initialized by dllmain thread
extern HINSTANCE gDllModule;
extern DInputProc Real_DirectInput8Create;

using namespace ModelMod;

static Uint16 InitialRepeatDelay = 500;
static Uint16 ContinuedRepeatDelay = 75;

namespace ModelMod {

Input::Input(void)
{
	_dinput = NULL;
	_lpdiKeyboard = NULL;
	memset(&_keyboardState,0,sizeof(_keyboardState));
	memset(&_lastKeyboardState,0,sizeof(_lastKeyboardState));
	memset(&_lastPressEvent,0,sizeof(_lastPressEvent));
	memset(&_repeatDelay,0,sizeof(_repeatDelay));
	_lastUpdate = 0;
	_altPressed = _shiftPressed = _ctrlPressed = false;
}

Input::~Input(void)
{
}

// called from update() as needed
bool Input::init() {
	HRESULT hr;
	if (!Real_DirectInput8Create) {
		MM_LOG_INFO(format("Process does not use DInput, creating my own"));
		hr = DirectInput8Create(gDllModule, DIRECTINPUT_VERSION, IID_IDirectInput8A, (LPVOID*)&_dinput, NULL);
	} else {
		hr = Real_DirectInput8Create(gDllModule, DIRECTINPUT_VERSION, IID_IDirectInput8A, &_dinput, NULL);
	}

	if (FAILED(hr)) {
		MM_LOG_INFO(format("Error: Failed to create DInput"));
	} else {
		hr = _dinput->CreateDevice(GUID_SysKeyboard, &_lpdiKeyboard, NULL); 
		if (FAILED(hr)) {
			MM_LOG_INFO("Error: Failed to create DInput keyboard");
		} else {
			MM_LOG_INFO("Created DInput keyboard");

			hr = _lpdiKeyboard->SetDataFormat(&c_dfDIKeyboard);
			if (FAILED(hr)) {
				MM_LOG_INFO("Error: Failed to set keyboard data format");
			} else {
				hr = _lpdiKeyboard->Acquire();
				if (FAILED(hr)) {
					MM_LOG_INFO("Error: Failed to acquire keyboard");
				}
			}
		}
	}

	return SUCCEEDED(hr);
}

void Input::reset() {
	memset(&_keyboardState,0,sizeof(_keyboardState));
}

vector<Input::KeyEvent> Input::update() {
	vector<Input::KeyEvent> events;

	DWORD now = GetTickCount();
	if (_lastUpdate != 0 && (now - _lastUpdate) < 10) {
		return events;
	}
	_lastUpdate = now;

	reset();
	if (!isInitialized() && !init())
		return events;

	// Get state
	HRESULT hr = _lpdiKeyboard->GetDeviceState(sizeof(_keyboardState),&_keyboardState);
	if (FAILED(hr)) {
		// reacquire
		hr = _lpdiKeyboard->Acquire();
		if (FAILED(hr)) {
			MM_LOG_INFO("Error: Failed to reacquire keyboard");
		} else {
			hr = _lpdiKeyboard->GetDeviceState(sizeof(_keyboardState),&_keyboardState);
		}
	} 

	if (SUCCEEDED(hr)) {
		// first update modifiers
		_altPressed = (_keyboardState[DIK_LALT] & 0x80) > 0 || (_keyboardState[DIK_RALT] & 0x80) > 0;
		_shiftPressed = (_keyboardState[DIK_LSHIFT] & 0x80) > 0 || (_keyboardState[DIK_RSHIFT] & 0x80) > 0;
		_ctrlPressed = (_keyboardState[DIK_LCONTROL] & 0x80) > 0 || (_keyboardState[DIK_RCONTROL] & 0x80) > 0;

		for (int i = 0; i < sizeof(_keyboardState); ++i) {
			bool pressed = (_keyboardState[i] & 0x80) > 0;
			bool wasPressed = (_lastKeyboardState[i] & 0x80) > 0;
			bool newPress = pressed && !wasPressed;
			bool newRelease = !pressed && wasPressed;

			if (newPress) {
				// per-key repeat delay is probably overkill, but who cares.
				_repeatDelay[i] = InitialRepeatDelay;
			}

			bool repeat = !newPress && pressed && _lastPressEvent[i] != 0 && (now - _lastPressEvent[i]) >= _repeatDelay[i];
			if (repeat) {
				// switch to lower delay now
				_repeatDelay[i] = ContinuedRepeatDelay;
			}

			if (newPress || repeat) {
				events.push_back(KeyEvent(i, true));
				_lastPressEvent[i] = now;
			} else if (newRelease) {
				events.push_back(KeyEvent(i, false));
			}
		}
	}

	memcpy(&_lastKeyboardState,&_keyboardState,sizeof(_lastKeyboardState));

	return events;
}

bool Input::isInitialized() {
	return _dinput != NULL;
}

};