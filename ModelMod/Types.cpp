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