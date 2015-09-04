#include "stdafx.h"

#include "Util.h"

namespace Util {
	void StrLower(string& s) {
		std::transform(s.begin(), s.end(), s.begin(), ::tolower);
	}

	int DisplayMessageBox(const char* text, const char* caption, UINT type) {
#ifdef UNICODE
		const size_t tLen = strlen(text);
		const size_t cLen = strlen(caption);
		WCHAR* wText = new WCHAR[tLen + 1];
		WCHAR* wCaption = new WCHAR[cLen + 1];
		MultiByteToWideChar(CP_OEMCP, 0, text, -1, wText, tLen + 1);
		MultiByteToWideChar(CP_OEMCP, 0, caption, -1, wCaption, cLen + 1);

		int res = MessageBox(NULL,
			wText,
			wCaption,
			type);

		delete[] wText;
		delete[] wCaption;
#else
		int res = MessageBox(NULL,
			text,
			caption,
			type);
#endif
		return res;
	};

	string Basename(string pathName) {
		string basename;
		const char* lastSlash = strrchr(pathName.c_str(), '\\');
		if (!lastSlash) {
			lastSlash = strrchr(pathName.c_str(), '/');
		}
		if (lastSlash) {
			basename = string(++lastSlash);
		}
		else {
			basename = string(pathName);
		}
		return basename;
	}

	// http://stackoverflow.com/questions/4643512/replace-substring-with-another-substring-c
	string ReplaceString(std::string subject, const std::string& search,
		const std::string& replace) {
		size_t pos = 0;
		while ((pos = subject.find(search, pos)) != std::string::npos) {
			subject.replace(pos, search.length(), replace);
			pos += replace.length();
		}
		return subject;
	}

};