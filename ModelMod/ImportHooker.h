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

#pragma once

#include <WinDef.h>

#include <string>
#include <map>
#include <vector>
using namespace std;

typedef struct ImpFunctionData {
	string name;
	DWORD origAddress;
	DWORD hookFnAddress;
} ImpFunctionData;

typedef map<string,ImpFunctionData> ImpFunctionDataMap;
typedef map<string,ImpFunctionDataMap> ImportMap;

/// Replace key global functions with our own variants.  This assumes that 
/// the functions are in the PE table of the main executable; NOT in some dll loaded
/// by that executable.  That is known to fail in some cases, in particular games that
/// load a secondary DLL that has the d3d9 imports.  Only way to hook those is to
/// extend this class to examine those PE tables as well or make this dll masquerade as d3d9.
/// Both are Ugh.  Maybe there are other methods - I'm no expert.
class ImportHooker
{
	ImportMap _imports;
public:
	ImportHooker(void);

	virtual ~ImportHooker(void);

	void add(string dll, string func, DWORD hookFnAddress);
	const ImpFunctionData* get(string dll, string func);

	void hook();
};

