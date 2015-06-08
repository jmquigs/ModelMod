#include "Log.h"

#include <windows.h> // for OutputDebugStringA

#include <cassert>

ModelMod::Log* ModelMod::Log::_sInstance = NULL;
namespace ModelMod {

Log::Log(void) :
	_level(Log::LOG_INFO), 
	_outputDebug(true),
	_outputFile(true),
	_fileReopen(true),
	_firstOpen(true),
	_fout(NULL) {
	memset(&_critSection, 0, sizeof(_critSection));
	InitializeCriticalSection(&_critSection);
}

Log::~Log(void) {
}

Log& Log::get() {
	// Note, we don't have the critical section yet, so we can't protect the lazy 
	// initialization from multiple threads.  However, this is
	// called very early from a single thread (dllmain Init(); aka the hook thread).  
	// Game threads can't even get in here until the hook thread completes at least part of its work, 
	// and that happens after log init.  So, this should be ok.
	if (_sInstance == NULL)
		_sInstance = new Log();
	return *_sInstance;
}

class CriticalSectionHandler {
	LPCRITICAL_SECTION _section;

public:
	CriticalSectionHandler(LPCRITICAL_SECTION section) {
		_section = section;
		EnterCriticalSection(_section);
	}
	virtual ~CriticalSectionHandler() {
		LeaveCriticalSection(_section);
	}
};

void Log::init(HMODULE callingDll) {
	CriticalSectionHandler cSect(&_critSection);

	// TODO: wide char support; ugh, also need it for fopen...

	// init log file in module directory
	string sBaseDir = "";
	{
		char baseModDirectory[8192];
		GetModuleFileName(callingDll, baseModDirectory, sizeof(baseModDirectory));
		char* lastBS = strrchr(baseModDirectory, '\\');
		if (lastBS != NULL) {
			*lastBS = 0;
		}
		sBaseDir = baseModDirectory;
	}

	// include the name of the executable in the log file name
	string sExeName = "unknownexe";
	{
		char exeName[8192];
		GetModuleFileName(NULL, exeName, sizeof(exeName));
		char* lastBS = strrchr(exeName, '\\');
		if (lastBS != NULL) {
			lastBS = lastBS++;
			if (*lastBS != NULL) {
				sExeName = string(lastBS);
			}
		}
	}
	
	_logFilePath = sBaseDir + "\\modelmod." + sExeName + ".log";
}

void Log::info(string message, string category, int cap) {
	_do_log(Log::LOG_INFO, message,category, cap);
}

void Log::setCategoryLevel(string category, int level) {
	CriticalSectionHandler cSect(&_critSection);

	_categoryLevel[category] = level;
}

int Log::getCategoryLevel(string category) {
	CriticalSectionHandler cSect(&_critSection); // this function is read-only, but map not guaranteed to be thread-safe

	map<string,int>::iterator iter = _categoryLevel.find(category);

	if (iter != _categoryLevel.end())
		return iter->second;
	else
		return -1;
}

void Log::_do_log(int level, const string& message, const string& category, int limit) {
	CriticalSectionHandler cSect(&_critSection);

	string messageSuffix = "";
	if (limit > 0) {
		int currCount = _limitedMessages[message];
		currCount++;
		// lets handle (unlikely) rollover gracefully 
		if (currCount < 0) {
			currCount = limit+1;
		}

		_limitedMessages[message] = currCount;

		if (currCount == limit) {
			messageSuffix = message + " (Final message; log limit hit)";
		}
		if (currCount > limit) {
			return;
		}
	}

	int catLevel = getCategoryLevel(category);
	if (catLevel != -1 && level < catLevel) 
		return;

	if (level < _level) 
		return;

	string lMsg = "[" + category + "]: " + message + messageSuffix;
	if (_outputDebug)
		_output_debug_string(lMsg);
	if (_outputFile)
		_output_file_string(lMsg);
}
	
void Log::_output_debug_string(const string& msg) {
	OutputDebugStringA(msg.c_str());
	OutputDebugStringA("\r\n");
}

void Log::_output_file_string(const string& msg) {
	CriticalSectionHandler cSect(&_critSection);

	if (_logFilePath.empty()) {
		// init with default settings
		init(NULL);
	}
	if (!_fout || _fileReopen) {
		// about to (re)open, handle should be null
		assert(_fout == NULL);

		if (_firstOpen)
			fopen_s(&_fout, _logFilePath.c_str(), "w");
		else
			fopen_s(&_fout, _logFilePath.c_str(), "a");
		_firstOpen = false;
	}

	if (_fout) {
		fputs(msg.c_str(), _fout);
		fputs("\n", _fout);
		if (_fileReopen) {
			fclose(_fout);
			_fout = NULL;
		}
		else {
			// leave file open
			//fflush(_fout); // SLOWWWW
		}
	}
}

};