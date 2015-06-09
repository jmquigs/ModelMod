#pragma once

#define WIN32_LEAN_AND_MEAN             // Exclude rarely-used stuff from Windows headers
// Windows Header Files:
#include <windows.h>

#include <string>
using namespace std;

#include <map>
using namespace std;

#define FMT_EXCEPTIONS 0 
#include "vendor/cppformat/format.h"
using namespace fmt;

namespace ModelMod {

class Log {
	static Log* _sInstance;

	int _level;
	map<string,int> _categoryLevel;
	map<string, int> _limitedMessages;

	bool _outputDebug;
	bool _outputFile;
	bool _fileReopen;
	bool _firstOpen;

	wstring _logFilePath;

	CRITICAL_SECTION _critSection;

	FILE* _fout;

public:
	// native code only has support for one log level.  I think that I have been pretty good 
	// about marking errors/warnings as such in error messages. (e.g. "Error: ")
	enum {
	    LOG_INFO
	};

	Log(void);

	virtual ~Log(void);

	static Log& get();

	void init(HMODULE callingDll);
	void info(string message, string category, int limit=0);

	void setCategoryLevel(string category, int level);

	int getCategoryLevel(string category);

private:
	
	void _do_log(int level, const string& message, const string& category, int limit);

	void _output_debug_string(const string& msg);
	void _output_file_string(const string& msg);
};

// The logging macros allow logging to be completely disabled (including overhead of processing arguments).
// This is useful in performance-critical code, but be careful that your arguments do not cause side-effects,
// since the code will be compiled-out if the macros are disabled.
#define MODELMOD_ENABLE_LOGGING_MACROS
#ifdef MODELMOD_ENABLE_LOGGING_MACROS
#define MM_LOG_INFO_LIMIT(msg,n) ModelMod::Log::get().info(msg,LogCategory,n);
#define MM_LOG_INFO(m) ModelMod::Log::get().info(m,LogCategory)
#define MM_LOG_INFO_CAT(m,c) ModelMod::Log::get().info(m,c)
#else
#define MM_LOG_INFO(m)
#define MM_LOG_INFO_CAT(m,c)
#endif

}; // namespace ModelMod