// dllmain.cpp : Defines the entry point for the DLL application.
#include "stdafx.h"

#include <Unknwn.h>

#include <stdio.h>
#include <string.h>

#include "Hook_IDirect3D9.h"
#include "Log.h"
#include "ImportHooker.h"

#include "Interop.h"
#include "Util.h"

#include <string>
using namespace std;

const string LogCategory="DllMain";

void SpinWhileFileExists(HMODULE dllModule) {
	// look for spin file in module directory
	char thisFilePath[8192];
	GetModuleFileName(dllModule, thisFilePath, sizeof(thisFilePath));
	char* lastBS = strrchr(thisFilePath, '\\');
	if (lastBS != NULL) {
		*lastBS = 0;
	}
	char spinPath[8192];
	sprintf_s(spinPath, sizeof(spinPath), "%s\\spin.txt", thisFilePath);

	// Consume CPU while waiting for debugger attach.  I like this better than using IsDebuggerPresent because
	// I can just create or delete the file when I need the debugger, rather than doing a code or build config modification

	// Note, to use this with Loader, you probably need to make a loader modification; search for
	// SpinWhileFileExists in inject.cpp.

	MM_LOG_INFO(format("Beginning spin: {}", spinPath));
	FILE* fp = NULL;
	do  {
		fopen_s(&fp, spinPath, "r");
		if (fp)
			fclose(fp);
		Sleep(1);
	}
	while (fp != NULL);	
}

DInputProc Real_DirectInput8Create = NULL;
HRESULT WINAPI Hook_DirectInput8Create(HINSTANCE hinst, DWORD dwVersion, REFIID riidltf, LPVOID *ppvOut, LPUNKNOWN punkOuter) {
	MM_LOG_INFO("DirectInput8Create called");
	if (Real_DirectInput8Create) {
		return Real_DirectInput8Create(hinst,dwVersion,riidltf,ppvOut,punkOuter);
	}
	return -1;
}

typedef HMODULE (WINAPI *LoadLibraryAProc)(__in LPCSTR lpLibFileName);

LoadLibraryAProc Real_LoadLibraryA = NULL;
HMODULE WINAPI Hook_LoadLibraryA(__in LPCSTR lpLibFileName) {
	MM_LOG_INFO(format("LoadLibraryA called: {}", lpLibFileName));
	if (Real_LoadLibraryA) {
		return Real_LoadLibraryA(lpLibFileName);
	}
	return NULL;
}

typedef HMODULE (WINAPI *LoadLibraryWProc)(__in LPCWSTR lpLibFileName);

LoadLibraryWProc Real_LoadLibraryW = NULL;
HMODULE WINAPI Hook_LoadLibraryW(__in LPCWSTR lpLibFileName) {
	// converting strings on windows
	// http://msdn.microsoft.com/en-us/library/ms235631(v=vs.80).aspx
	
    size_t origsize = wcslen(lpLibFileName) + 1;
    const size_t newsize = 4096;
    size_t convertedChars = 0;
    char nstring[newsize];
    wcstombs_s(&convertedChars, nstring, origsize, lpLibFileName, _TRUNCATE);

	MM_LOG_INFO(format("LoadLibraryW called: {}", nstring));
	if (Real_LoadLibraryW) {
		return Real_LoadLibraryW(lpLibFileName);
	}
	return NULL;
}

bool gLazyInitted = false;
HMODULE gDllModule = NULL;
Hook_IDirect3D9* gH_D3D9 = NULL;

void PrepCLR() {
	const int PathSize = 8192;
	char* srcBytes = NULL;
	char* destBytes = NULL;

	MM_LOG_INFO("Preparing for CLR launch");

	WCHAR mmDllPath[PathSize];
	GetModuleFileNameW(gDllModule, mmDllPath, sizeof(mmDllPath));
	WCHAR* lastBS = wcsrchr(mmDllPath, '\\');
	if (lastBS != NULL) {
		*lastBS = 0;
	}
	WCHAR sourcePath[PathSize];
	// divide by size of element zero; required for this booby-trapped "secure" function
	swprintf_s(sourcePath, sizeof(sourcePath)/sizeof(sourcePath[0]), L"%s\\ModelModCLRAppDomain.dll", mmDllPath);

	WCHAR currDir[PathSize];
	::GetCurrentDirectoryW(sizeof(currDir), currDir);
	WCHAR destPath[PathSize];
	swprintf_s(destPath, sizeof(destPath)/sizeof(destPath[0]), L"%s\\ModelModCLRAppDomain.dll", currDir);

	DeleteFileW(destPath);
	BOOL ok = CopyFileW(sourcePath, destPath, TRUE);

	if (!ok) {
		char *mbSrc = ModelMod::Util::convertToMB(sourcePath);
		char* mbDest = ModelMod::Util::convertToMB(destPath);
		MM_LOG_INFO(format("Error: failed to copy MM app domain file from {} to {}, cannot init CLR", mbSrc, mbDest));
		delete [] mbSrc;
		delete [] mbDest;
		return;
	} else {
		MM_LOG_INFO("Copied app domain file into game dir");
	}

	HANDLE srcH = INVALID_HANDLE_VALUE;
	HANDLE destH = INVALID_HANDLE_VALUE;

	// Verify that the dest file is really what we expect.  This is a bit of wet-paper-bag security checking.
	destH = CreateFileW(destPath, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_READONLY, NULL);
	if (destH == INVALID_HANDLE_VALUE) {
		MM_LOG_INFO("Error: failed to open output app domain file after copy");
		goto Cleanup;
	}

	srcH = CreateFileW(sourcePath, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_READONLY, NULL);
	if (srcH == INVALID_HANDLE_VALUE) {
		MM_LOG_INFO("Error: failed to open source app domain file after copy");
		goto Cleanup;
	}

	DWORD srcSize = GetFileSize(srcH, NULL);
	DWORD destSize = GetFileSize(destH, NULL);

	if (srcSize == 0xFFFFFFFF || destSize == 0xFFFFFFFF) {
		MM_LOG_INFO("Error: failed to obtain source or dest file size after copy");
		goto Cleanup;
	}

	if (srcSize != destSize) {
		MM_LOG_INFO("Error: source size != dest size after copy");
		goto Cleanup;
	}

	const int MaxAppDomainSize = 50000;
	if (srcSize > MaxAppDomainSize || destSize > MaxAppDomainSize) {
		MM_LOG_INFO("Error: app domain file exceeded hardcoded limit for load");
		goto Cleanup;
	}

	srcBytes = new char[MaxAppDomainSize];
	destBytes = new char[MaxAppDomainSize];

	DWORD srcBytesRead;
	DWORD destBytesRead;
	if (!ReadFile(srcH, srcBytes, srcSize, &srcBytesRead, NULL)) {
		MM_LOG_INFO("Error: failed to read source app domain file");
		goto Cleanup;
	}
	if (!ReadFile(destH, destBytes, destSize, &destBytesRead, NULL)) {
		MM_LOG_INFO("Error: failed to read dest app domain file");
		goto Cleanup;
	}

	if (srcBytesRead != srcSize) {
		MM_LOG_INFO("Error: failed to read all source bytes");
		goto Cleanup;
	}
	if (destBytesRead != destSize) {
		MM_LOG_INFO("Error: failed to read all dest bytes");
		goto Cleanup;
	}
	if (memcmp(srcBytes,destBytes,destBytesRead) != 0) {
		MM_LOG_INFO("Error: bytes differ after app domain copy");
		goto Cleanup;
	}

	// if we survive all that, then source and dest are the same and in theory the dest file is locked so that it can't be overwritten now.
	// therefore we can load the CLR, which will load (and lock) the dest app domain file.
	// of course one weak link here is someone managing to change the source file to something other than
	// we expect, but if they can do that, they could probably also swap out this DLL as well.  
	int ret = Interop::InitCLR(mmDllPath);
	MM_LOG_INFO(format("Init CLR returned: {:x}", ret));

Cleanup:
	if (srcH != INVALID_HANDLE_VALUE) {
		CloseHandle(srcH);
	}
	if (destH != INVALID_HANDLE_VALUE) {
		CloseHandle(destH);
	}

	delete [] srcBytes;
	delete [] destBytes;
}

void LazyInit() {
	if (gLazyInitted) {
		return;
	}

	gLazyInitted = true;

	MM_LOG_INFO(format("Starting Lazy Init"));

	PrepCLR();

	MM_LOG_INFO(format("Finished Lazy Init"));
}

struct IDirect3D9;

typedef IDirect3D9* (WINAPI *Direct3DCreate9Proc)(UINT SDKVersion);

Direct3DCreate9Proc Real_Direct3DCreate9 = NULL;

IDirect3D9* WINAPI Hook_Direct3DCreate9(UINT SDKVersion) {
	IDirect3D9* rd3d9 = NULL;
	if (Real_Direct3DCreate9) {
		MM_LOG_INFO(format("Direct3DCreate9 called"));
		LazyInit();

		MM_LOG_INFO(format("Replacing d3d9 with hook interface"));
		rd3d9 = Real_Direct3DCreate9(SDKVersion);
		if (gH_D3D9) {
			MM_LOG_INFO(format("Application requested another d3d9 interface, allocating another hook interface"));
			// Some apps create multiple d3d9 interfaces and hang on to them.  Thus, it isn't valid to release the interface
			// just because an app asked for another one (could crash the app).  So just replace the old one without
			// deallocating (potential leak)
			// TODO: maybe I should just AddRef on it, then Release() here?
			//MM_LOG_INFO(format("Releasing old hook interface"));
			//gH_D3D9->Release();
			//delete gH_D3D9;
		}
		gH_D3D9 = new Hook_IDirect3D9(rd3d9);
		return gH_D3D9;
	}
	return NULL;
}


typedef FARPROC (WINAPI *GetProcAddressProc) (__in HMODULE hModule, __in LPCSTR lpProcName);
GetProcAddressProc Real_GetProcAddress = NULL;
FARPROC WINAPI Hook_GetProcAddress(__in HMODULE hModule, __in LPCSTR lpProcName) {
	MM_LOG_INFO(format("GetProcAddress: {}", lpProcName));
	string sProc(lpProcName);
	if (sProc == "Direct3DCreate9") {
		Real_Direct3DCreate9 = (Direct3DCreate9Proc)Real_GetProcAddress(hModule,lpProcName);
		return (FARPROC)Hook_Direct3DCreate9;
	}
	if (Real_GetProcAddress) {
		return Real_GetProcAddress(hModule,lpProcName);
	}
	return NULL;
};

void Init(HMODULE dllModule) {
	Log::get().init(dllModule);
	MM_LOG_INFO("Log Initialized");

	SpinWhileFileExists(dllModule);

	gDllModule = dllModule;

	ImportHooker hooker;
	hooker.add("d3d9.dll", "Direct3DCreate9", (DWORD)Hook_Direct3DCreate9);
	hooker.add("dinput8.dll", "DirectInput8Create", (DWORD)Hook_DirectInput8Create);
	hooker.add("kernel32.dll", "LoadLibraryA", (DWORD)Hook_LoadLibraryA);
	hooker.add("kernel32.dll", "LoadLibraryW", (DWORD)Hook_LoadLibraryW);
	hooker.add("kernel32.dll", "GetProcAddress", (DWORD)Hook_GetProcAddress);
	hooker.hook();

	const ImpFunctionData* data = hooker.get("dinput8.dll", "DirectInput8Create");
	if (data && data->origAddress) {	
		Real_DirectInput8Create = (DInputProc)data->origAddress; 
	}
	data = hooker.get("kernel32.dll", "GetProcAddress");
	if (data && data->origAddress) {	
		Real_GetProcAddress = (GetProcAddressProc)data->origAddress; 
	}
	data = hooker.get("kernel32.dll", "LoadLibraryA");
	if (data && data->origAddress) {	
		Real_LoadLibraryA = (LoadLibraryAProc)data->origAddress; 
	}
	data = hooker.get("kernel32.dll", "LoadLibraryW");
	if (data && data->origAddress) {	
		Real_LoadLibraryW = (LoadLibraryWProc)data->origAddress; 
	}
	data = hooker.get("d3d9.dll", "Direct3DCreate9");
	if (data && data->origAddress) {
		Real_Direct3DCreate9 = (Direct3DCreate9Proc)data->origAddress;
	}

	FlushInstructionCache(GetCurrentProcess(), NULL, 0);
	MM_LOG_INFO("DLL Init complete");
}

BOOL APIENTRY DllMain( HMODULE hModule,
                       DWORD  ul_reason_for_call,
                       LPVOID lpReserved
					 )
{
	switch (ul_reason_for_call)
	{
	case DLL_PROCESS_ATTACH:
		Init(hModule);
		break;
	case DLL_THREAD_ATTACH:
	case DLL_THREAD_DETACH:
	case DLL_PROCESS_DETACH:
		break;
	}
	return TRUE;
}

