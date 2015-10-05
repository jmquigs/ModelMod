// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

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