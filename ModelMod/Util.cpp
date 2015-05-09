#include "Util.h"

#include <algorithm>
#include "Log.h"

#include <cstdio>

namespace ModelMod {

string LogCategory = "Util";

string Util::toLowerCase(string s) {
	std::transform(s.begin(), s.end(), s.begin(), ::tolower);
	return s;
}

bool Util::startsWith (std::string const &fullString, std::string const &starting) {
	if (fullString.length() >= starting.length()) {
        return (0 == fullString.compare (0, starting.length(), starting));
    } else {
        return false;
    }
}
bool Util::endsWith (std::string const &fullString, std::string const &ending) {
    if (fullString.length() >= ending.length()) {
        return (0 == fullString.compare (fullString.length() - ending.length(), ending.length(), ending));
    } else {
        return false;
    }
}

char* Util::convertToMB(wchar_t* src) {
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
}