#pragma once

#include <string>
using namespace std;

class Inject
{
	string _injectError;

public:
	Inject(void);
	~Inject(void);

	string GetError() { return _injectError; }
	bool InjectDLL(DWORD processId, const char * dllPath, bool processWasLaunched);

private:
	bool DoInjectDLL(DWORD processId, const char * dllPath, bool processWasLaunched);

};

