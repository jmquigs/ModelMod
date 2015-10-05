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

