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

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include "ImportHooker.h"

#include "Log.h"
#include "Util.h"
using namespace ModelMod;

#include <string> 

const string LogCategory("ImportHooker");

typedef unsigned int UInt32;

void SafeWrite32(UInt32 addr, UInt32 data)
{
	DWORD	oldProtect;

	BOOL res = VirtualProtect((void *)addr, 4, PAGE_EXECUTE_READWRITE, &oldProtect);;
	MM_LOG_INFO(format("Virtual Protect: {}", res));
	if (res) {
		MM_LOG_INFO("About to write");
		*((UInt32 *)addr) = data;
		MM_LOG_INFO("Done writing memory");
		res = VirtualProtect((void *)addr, 4, oldProtect, &oldProtect);
		if (!res) {
			MM_LOG_INFO(format("Error: Virtual Protect2 Failed: {:x}", GetLastError()));
		}
	} else {
		MM_LOG_INFO(format("Error: Virtual Protect1 Failed: {:x}", GetLastError()));
	}
}

ImportHooker::ImportHooker(void)
{
}

ImportHooker::~ImportHooker(void)
{
}

void ImportHooker::add(string dll, string func, DWORD hookFn) {
	dll = Util::toLowerCase(dll);
	func = Util::toLowerCase(func);

	ImpFunctionData data;
	data.hookFnAddress = hookFn;
	data.name = func;
	data.origAddress = 0;
	_imports[dll][func] = data;
}

const ImpFunctionData* ImportHooker::get(string dll, string func) {
	dll = Util::toLowerCase(dll);
	func = Util::toLowerCase(func);

	if (_imports.find(dll) == _imports.end()) {
		return NULL;
	}

	if (_imports[dll].find(func) == _imports[dll].end()) {
		return NULL;
	}

	return &(_imports[dll][func]);
}

void ImportHooker::hook() {
	HMODULE thisModule = GetModuleHandle(NULL);
	IMAGE_DOS_HEADER* dHdr = (IMAGE_DOS_HEADER*)(thisModule);
	DWORD ntHeaderBase = (DWORD)thisModule;
	ntHeaderBase += (DWORD)(dHdr->e_lfanew);
	DWORD moduleBase = (DWORD)thisModule;
	IMAGE_NT_HEADERS32* peHdr = (IMAGE_NT_HEADERS32*)(ntHeaderBase);
	IMAGE_SECTION_HEADER* sHdrs = (IMAGE_SECTION_HEADER*)((ntHeaderBase) + sizeof(IMAGE_NT_HEADERS32));
	DWORD importSection = 0;

	IMAGE_IMPORT_DESCRIPTOR* iSection = 
		(IMAGE_IMPORT_DESCRIPTOR*) (DWORD(thisModule) + DWORD(peHdr->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT].VirtualAddress));

	for ( ; iSection && iSection->OriginalFirstThunk > 0 ; ++iSection) {
		const char* sName = NULL;
		string strName;
		if (iSection->Name) {
			sName = (const char*)(moduleBase + iSection->Name);
			strName = Util::toLowerCase(sName);

			if (_imports.find(strName) == _imports.end()) {
				MM_LOG_INFO(format("Ignoring import library: {}", sName ));
				continue;
			} else {
				MM_LOG_INFO(format("Scanning import library: {}", sName ));
			}
		}
		ImpFunctionDataMap& fd = _imports[strName];
		if (fd.size() == 0) {
			MM_LOG_INFO("No functions hooked from import library");
			continue;
		}

		// walk thunks, looking for functions
		IMAGE_THUNK_DATA* td = (IMAGE_THUNK_DATA*)(moduleBase + iSection->OriginalFirstThunk);
		IMAGE_THUNK_DATA* tdIAT = (IMAGE_THUNK_DATA*)(moduleBase + iSection->FirstThunk);

		while (td->u1.AddressOfData != 0) {
			IMAGE_IMPORT_BY_NAME* iibn = (IMAGE_IMPORT_BY_NAME*)(moduleBase + td->u1.AddressOfData);
			string strIibn((const char*)iibn->Name);
			strIibn = Util::toLowerCase(strIibn);

			if (fd.find(strIibn) == fd.end()) {
				MM_LOG_INFO(format("Ignoring function: {}", (const char*)iibn->Name));
			} else {
				MM_LOG_INFO(format("Found function: {}", (const char*)iibn->Name));
				DWORD thunkAddress = (DWORD)(tdIAT);
				ImpFunctionData& data = fd[strIibn];
				data.origAddress = (DWORD)*(DWORD*)thunkAddress;

				SafeWrite32(thunkAddress,data.hookFnAddress);
			}

			td++;
			tdIAT++;
		}
	}

}