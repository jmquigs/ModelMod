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

	// http://stackoverflow.com/questions/874134/find-if-string-endswith-another-string-in-c
	bool HasEnding(string& fullString, string& ending) {
		if (fullString.length() >= ending.length()) {
			return (0 == fullString.compare(fullString.length() - ending.length(), ending.length(), ending));
		}
		else {
			return false;
		}
	}

	FILE* log_handle = stdout;

	void SetLogFile(FILE* fp) {
		if (fp != NULL) {
			log_handle = fp;
		}
	}

	void Log(const char * fmt, ...)
	{
		if (log_handle) {
			va_list args;

			va_start(args, fmt);
			vfprintf(log_handle, fmt, args);
			// this doesn't seem to flush the buffer if its a file, even though I specified "c" to fopen.
			// don't want to close/reopen the file here because that could slow things down, 
			// and the injection is definitely timing sensitive,
			// so just avoid flush altogether.
			//fflush(log_handle);
			//_flushall();
			va_end(args);
		}
	}

	// Duplicated from modelmod's Util, because MMLoader doesn't directly include ModelMod code
	char* Util::ConvertToMB(wchar_t* src) {
		if (!src) {
			return NULL;
		}

		const size_t maxSize = 16384;
		char* out = new char[16384];
		out[0] = 0;
		size_t numConverted;
		wcstombs_s(&numConverted, out, maxSize, src, maxSize);
		return out;
	}
};