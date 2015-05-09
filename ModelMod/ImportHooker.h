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

