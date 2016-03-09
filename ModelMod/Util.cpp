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

Uint8* Util::slurpFile(LPCWSTR filename, Uint32& outSize) {
	// read file data
	HANDLE in = INVALID_HANDLE_VALUE;
	bool readOk = false;
	Uint8* data = NULL;

	InvokeOnDrop drop([&]() {
		if (in != INVALID_HANDLE_VALUE) {
			CloseHandle(in);
		}
		if (!readOk) {
			delete[] data;
		}
	});

	in = CreateFileW(filename, GENERIC_READ, 0, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
	if (in == INVALID_HANDLE_VALUE) {
		MM_LOG_INFO(format("Failed to open input file: {}", GetLastError()));
		return NULL;
	}

	LARGE_INTEGER fsize; // windows has funny type names
	if (!GetFileSizeEx(in, &fsize)) {
		MM_LOG_INFO(format("Failed to get input file size: {}", GetLastError()));
		return NULL;
	}

	if (fsize.QuadPart > MAXDWORD) {
		MM_LOG_INFO(format("File too large!"));
		return NULL;
	}

	outSize = (DWORD)fsize.QuadPart;
	data = new Uint8[outSize];

	DWORD numRead = 0;
	if (!ReadFile(in, data, outSize, &numRead, NULL)) {
		MM_LOG_INFO(format("Failed to read file: {}", GetLastError()));
		return NULL;
	}

	if (numRead != outSize) {
		MM_LOG_INFO(format("Failed to read file: expected {} bytes, but only got {}", outSize, numRead));
		return NULL;
	}

	readOk = true;
	return data;
}

}