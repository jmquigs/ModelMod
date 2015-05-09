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
}

Log::~Log(void) {
}

Log& Log::get() {
	if (_sInstance == NULL)
		_sInstance = new Log();
	return *_sInstance;
}

void Log::init(HMODULE callingDll) {
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

void Log::info(string message, string category) {
	_do_log(Log::LOG_INFO, message,category);
}

void Log::setCategoryLevel(string category, int level) {
	_categoryLevel[category] = level;
}

int Log::getCategoryLevel(string category) {
	map<string,int>::iterator iter = _categoryLevel.find(category);

	if (iter != _categoryLevel.end())
		return iter->second;
	else
		return -1;
}

void Log::_do_log(int level, string& message, string& category) {
	int catLevel = getCategoryLevel(category);
	if (catLevel != -1 && level < catLevel)
		return;

	if (level < _level)
		return;

	string lMsg = "[" + category + "]: " + message;
	if (_outputDebug)
		_output_debug_string(lMsg);
	if (_outputFile)
		_output_file_string(lMsg);
}
	
void Log::_output_debug_string(string& msg) {
	OutputDebugStringA(msg.c_str());
	OutputDebugStringA("\r\n");
}

void Log::_output_file_string(string& msg) {
	if (_logFilePath.empty()) {
		// init with default settings
		init(NULL);
	}
	if (!_fout || _fileReopen) {
		assert(!_fout);
		if (_firstOpen)
			fopen_s(&_fout, _logFilePath.c_str(), "w");
		else
			fopen_s(&_fout, _logFilePath.c_str(), "a");
		_firstOpen = false;
	}

	if (_fout) {
		fputs(msg.c_str(), _fout);
		fputs("\n", _fout);
		if (!_fileReopen) {
			//fflush(_fout);
		}
		else {
			fclose(_fout);
			_fout = NULL;
		}
	}
}

};