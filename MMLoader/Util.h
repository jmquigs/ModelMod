#pragma once

#include <algorithm>
#include <string>
using namespace std;

namespace Util {
	int DisplayMessageBox(const char* text, const char* caption = "Caption", UINT type = MB_OK);

	void StrLower(string& s);
	string Basename(string pathName);
	string ReplaceString(std::string subject, const std::string& search,
		const std::string& replace);
	bool HasEnding(string& fullString, string& ending);
	char* ConvertToMB(wchar_t* src);

	void SetLogFile(FILE* fp);
	void Log(const char * fmt, ...);
};