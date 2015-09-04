#pragma once

#include <algorithm>
#include <string>
using namespace std;

namespace Util {
	void StrLower(string& s);

	int DisplayMessageBox(const char* text, const char* caption = "Caption", UINT type = MB_OK);

	string Basename(string pathName);

	string ReplaceString(std::string subject, const std::string& search,
		const std::string& replace);
};