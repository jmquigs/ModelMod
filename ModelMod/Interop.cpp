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

#include "stdafx.h"
#include "Interop.h"
#include <metahost.h>
#include "HostControl.h"
#include <comutil.h>
#include <CorError.h>

#include "Types.h"
#include "Log.h"
#include "Util.h"
#include "RenderState.h"

using namespace ModelMod;

const string LogCategory = "NativeInterop";

// thread-unsafe global state
int gInteropStatus = 0;
WCHAR gMMPath[8192] = { 0 };
IMMAppDomainMananger* gDomainManager = NULL;
ManagedCallbacks gCurrentCallbacks;
ConfData gCurrentConf;
bool gCallbacksInitialized = false;

// end global state

INTEROP_API int GetMMVersion() {
	return 0;
}

const int RetFailNoManager = 1;
const int RetFailNoPath = 2;
const int RetFailAssemblyLoadFailure = 3;
const int RetFailGotManager = 4;

namespace Interop {

int InitCLR(WCHAR* mmPath) {
	if (gDomainManager) {
		MM_LOG_INFO("Error: Domain manager already initialized; don't call InitCLR() again");
		return RetFailGotManager;
	}

	// Note, this function allocate a bunch of COM pointers and then essentially 
	// leaks them (in both success and failure cases).  However this function is 
	// just executed once and the resulting objects
	// are all singletons anyway, so process-exit style cleanup is fine here.
	// IF we ever switch to invoking this more than once, however, we'll need 
	// to track and clean up those objects.

	if (!mmPath) {
		MM_LOG_INFO("Error: No path specified, can't load managed assembly");
		return RetFailNoPath;
	}
	swprintf_s(gMMPath, sizeof(gMMPath)/sizeof(gMMPath[0]), mmPath);

	MM_LOG_INFO("Searching for CLR");

	HRESULT hr;
	ICLRMetaHost* metaHost;
	hr = CLRCreateInstance(CLSID_CLRMetaHost,  IID_ICLRMetaHost, (LPVOID*)&metaHost);
	if (FAILED(hr)) {
		MM_LOG_INFO("Failed to create clr meta host instance");
		return hr;
	}

	IEnumUnknown *runtimeEnum;
	hr = metaHost->EnumerateInstalledRuntimes((IEnumUnknown **)&runtimeEnum);
	if (FAILED(hr)) {
		MM_LOG_INFO("Failed to enumerate installed clr runtimes");
		return hr;
	}

	ICLRRuntimeInfo* rt = NULL;
	ULONG fetched = 0;
	ICLRRuntimeInfo* targetRuntime = NULL;
	WCHAR wvStr[1024];
	DWORD wvStrsize = 1024;
	while (targetRuntime == NULL && SUCCEEDED(runtimeEnum->Next(1, (IUnknown**) &rt, &fetched)) && rt != NULL) {
		
		hr = rt->GetVersionString(wvStr, &wvStrsize);
		if (SUCCEEDED(hr)) {
			BOOL loadable = false;
			hr = rt->IsLoadable(&loadable);
			if (FAILED(hr)) {
				MM_LOG_INFO("Failed to check whether a clr runtime is loadable");
			}
			// lets target v4.0 for now
			if (wcsstr(wvStr, L"v4.0") == wvStr) {
				if (loadable) {
					targetRuntime = rt;
				}
			}
		}
	}

	runtimeEnum->Release();
	
	if (!targetRuntime) {
		MM_LOG_INFO("Failed to locate a loadable clr runtime for target version");
		return hr;
	}

	// log version of discovered runtime; wait until after we create it to log its installation directory,
	// since that doesn't appear to be available yet.
	{
		char* rtVer = Util::convertToMB(L"Unknown");

		WCHAR wStr[8192];
		DWORD wStrsize = 8192;
		HRESULT hr = targetRuntime->GetVersionString(wStr, &wStrsize);
		if (FAILED(hr)) {
			MM_LOG_INFO("Failed to get target runtime version string");
		}
		else {
			rtVer = Util::convertToMB(wStr);
		}

		MM_LOG_INFO(fmt::format("Found CLR runtime version {}", rtVer));
		delete[] rtVer;
	}

	ICLRRuntimeInfo* runtime = NULL;
	hr = metaHost->GetRuntime(wvStr,  IID_ICLRRuntimeInfo, (LPVOID*)&runtime);
	if (FAILED(hr) || runtime == NULL) {
		MM_LOG_INFO("Failed to obtain a runtime for loaded clr");
		return hr;
	}

	// log the runtime's installation directory
	{
		char* rtDir = Util::convertToMB(L"Unknown");

		WCHAR wStr[8192];
		DWORD wStrsize = 8192;
		hr = runtime->GetRuntimeDirectory(wStr, &wStrsize);
		if (FAILED(hr)) {
			MM_LOG_INFO("Failed to get target runtime directory");
		}
		else {
			rtDir = Util::convertToMB(wStr);
		}

		MM_LOG_INFO(fmt::format("Found CLR in directory {}", rtDir));
		delete[] rtDir;
	}

	// The domain manager dll must be in the same directory as the executable, apparently.
	PCWSTR pszAppDomainAssemblyName = L"ModelModCLRAppDomain"; 
	PCWSTR pszAppDomainClass = L"MMAppDomain.MMAppDomainManager";

	ICLRRuntimeHost* rHost = NULL;
	hr = runtime->GetInterface(CLSID_CLRRuntimeHost,  IID_ICLRRuntimeHost, (LPVOID*) &rHost);
	if (FAILED(hr) || rHost == NULL) {
		MM_LOG_INFO("Failed to get the interface for the clr runtime host");
		return hr;
	}

	HostControl* mmHostControl = new HostControl();

	ICLRControl* clrControl = NULL;
	hr = rHost->GetCLRControl(&clrControl);
	if (FAILED(hr)) {
		MM_LOG_INFO("Failed to get the interface for the clr control");
		return hr;
	}
	hr = rHost->SetHostControl(mmHostControl);
	if (FAILED(hr)) {
		MM_LOG_INFO("Failed to set host control");
		return hr;
	}
	hr = clrControl->SetAppDomainManagerType(pszAppDomainAssemblyName, pszAppDomainClass);
	if (FAILED(hr)) {
		MM_LOG_INFO("Failed to set the app domain manager type");
		return hr;
	}

	MM_LOG_INFO("Starting CLR");
	hr = rHost->Start();
	if (FAILED(hr)) {
		MM_LOG_INFO("Failed to start the clr");
		switch (hr) {
		case HOST_E_CLRNOTAVAILABLE: 
			MM_LOG_INFO("clr is not available");
			break;
		case HOST_E_TIMEOUT:
			MM_LOG_INFO("call timed out");
			break;
		case HOST_E_NOT_OWNER:
			MM_LOG_INFO("caller does not own the lock");
			break;
		case HOST_E_ABANDONED:
			MM_LOG_INFO("event was canceled while a blocked thread was waiting");
			break;
		case E_FAIL:
			MM_LOG_INFO("unknown catastrophic error");
			break;
		default:
			MM_LOG_INFO(fmt::format("unknown error {}; offset from HOST_E_CLRNOTAVAILABLE: {}", hr, (hr - HOST_E_CLRNOTAVAILABLE)));
			break;
		}
		return hr;
	}

	// now should be able to get the app domain manager
	gDomainManager = mmHostControl->GetDomainMananger();
	if (!gDomainManager) {
		MM_LOG_INFO("Failed to obtain the clr domain manager");
		return hr;
	}

	return ReloadAssembly();
}

int ReloadAssembly() {
	if (!gDomainManager) {
		MM_LOG_INFO("Error: Domain manager not initialized; can't load managed assembly");
		return RetFailNoManager;
	}
	if (!gMMPath[0]) {
		MM_LOG_INFO("Error: No path specified, can't load managed assembly");
		return RetFailNoPath;
	}

	memset(&gCurrentCallbacks, 0, sizeof(gCurrentCallbacks));
	gCallbacksInitialized = false;

	// load the mm assembly
	WCHAR asmPath[8192];
	swprintf_s(asmPath, sizeof(asmPath)/sizeof(asmPath[0]), L"%s\\MMManaged.dll", gMMPath);

	char* mbAsmPath = Util::convertToMB(asmPath);
	MM_LOG_INFO(format("Loading managed assembly: {}", mbAsmPath));
	delete [] mbAsmPath;

	BSTR res = gDomainManager->Load(asmPath);
	char* cStr = _com_util::ConvertBSTRToString(res);
	// anything other than empty string is an error
	if (!cStr || cStr[0] != NULL) {
		MM_LOG_INFO(format("Failed to init CLR: {}", cStr));
		delete cStr;
		return RetFailAssemblyLoadFailure;
	}
	return 0;
}

bool OK() {
	return gDomainManager && gCallbacksInitialized;
}

const ManagedCallbacks& Callbacks() {
	assert(OK());
	return gCurrentCallbacks;
}

const ConfData& Conf() {
	return gCurrentConf;
}

}; //namespace managed

INTEROP_API int OnInitialized(ManagedCallbacks* callbacks) {
	gCurrentCallbacks = *callbacks;
	gCallbacksInitialized = true;

	WCHAR wExeModule[8192];
	GetModuleFileNameW(NULL, wExeModule, sizeof(wExeModule));

	ConfData* conf = callbacks->SetPaths(gMMPath, wExeModule);
	if (conf) {
		gCurrentConf = *conf;
	}
	else {
		MM_LOG_INFO("Error: no conf received from managed code");
		gCurrentConf = ConfData();
	}

	MM_LOG_INFO(fmt::format("Full run mode: {}", gCurrentConf.RunModeFull));
	MM_LOG_INFO(fmt::format("Load mods on start: {}", gCurrentConf.LoadModsOnStart));
	MM_LOG_INFO(fmt::format("Input profile: {}", gCurrentConf.InputProfile));

	return 0;
}

INTEROP_API void LogInfo(char* category, char* message) {
	ModelMod::Log::get().info(message,category);
}
INTEROP_API void LogWarn(char* category, char* message) {
	// only info() is available right now, so hack category name
	char newcat[1024];
	sprintf_s(newcat,sizeof(newcat), "WARN-%s", category);
	ModelMod::Log::get().info(message,newcat);
}
INTEROP_API void LogError(char* category, char* message) {
	// only info() is available right now, so hack category name
	char newcat[1024];
	sprintf_s(newcat,sizeof(newcat), "ERROR-%s", category);
	ModelMod::Log::get().info(message,newcat);
}

INTEROP_API void SaveTexture(int index, WCHAR* path) {
	RenderState::get().saveTexture(index,path);
}