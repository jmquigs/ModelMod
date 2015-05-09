#pragma once
#include <string>
using namespace std;

#include"Types.h"

namespace ModelMod {

class Util
{
public:
	static string toLowerCase(string s);

	static bool startsWith (std::string const &fullString, std::string const &starting);
	static bool endsWith (std::string const &fullString, std::string const &ending);

	// convert a wide string to multibyte.  Caller must delete[] the memory after use.
	// Result max size (in bytes) is 16384.  This function is intended only for one-off logging.
	static char* convertToMB(wchar_t* src);
};

};