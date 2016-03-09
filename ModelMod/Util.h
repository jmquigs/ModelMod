// ModelMod: 3d data snapshotting & substitution program.
// Copyright(C) 2015,2016 John Quigley

// This program is free software : you can redistribute it and / or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.See the
// GNU General Public License for more details.

// You should have received a copy of the GNU Lesser General Public License
// along with this program.If not, see <http://www.gnu.org/licenses/>.

#pragma once

#define WIN32_LEAN_AND_MEAN             // Exclude rarely-used stuff from Windows headers
// Windows Header Files:
#include <windows.h>

#include <string>
#include <functional>
using namespace std;

#include"Types.h"

namespace ModelMod {

// Invoke a function when this class goes out of scope.
// Bah, there are probably one or more "standard" ways to do this.
class InvokeOnDrop {
	std::function<void()> _fn;
public:
	InvokeOnDrop(std::function<void()> fn) {
		this->_fn = fn;
	}

	virtual ~InvokeOnDrop() {
		this->_fn();
	}
};

class Util
{
public:
	static string toLowerCase(string s);

	static bool startsWith (std::string const &fullString, std::string const &starting);
	static bool endsWith (std::string const &fullString, std::string const &ending);

	static Uint8* slurpFile(LPCWSTR filename, Uint32& outSize);

	// convert a wide string to multibyte.  Caller must delete[] the memory after use.
	// Result max size (in bytes) is 16384.  This function is intended only for one-off logging.
	static char* convertToMB(wchar_t* src);
};

};