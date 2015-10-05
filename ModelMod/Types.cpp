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

#include <map>
using namespace std;

#include "Types.h"
#include "Util.h"

namespace ModelMod {

static map<string,ModType> stringTypeMap;

ModType GetType(std::string sType) {
	if (stringTypeMap.size() == 0) {
		stringTypeMap["none"] = None;
		stringTypeMap["cpuadditive"] = CPUAdditive;
		stringTypeMap["cpureplacement"] = CPUReplacement;
		stringTypeMap["gpuperturbation"] = GPUPertubation;
		stringTypeMap["gpureplacement"] = GPUReplacement;
		stringTypeMap["deletion"] = Deletion;
	}

	sType = Util::toLowerCase(sType);
	if (!stringTypeMap.count(sType)) {
		return None;
	} else {
		return stringTypeMap[sType];
	}
}

string GetTypeString(ModType type) {
	switch (type) {
	case None:
		return "None";
	case CPUAdditive:
		return "CPUAdditive";
	case CPUReplacement:
		return "CPUReplacement";
	case GPUReplacement:
		return "GPUReplacement";
	case GPUPertubation:
		return "GPUPertubation";
	case Deletion:
		return "Deletion";
	}
	return "None";
}

};